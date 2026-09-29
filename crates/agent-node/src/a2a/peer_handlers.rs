use crate::*;
pub(crate) async fn client_connect(
    State(state): State<AppState>,
    headers: HeaderMap,
    peer: Option<axum::Extension<axum::extract::ConnectInfo<std::net::SocketAddr>>>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let credential = auth_client(&state, &headers).await?;
    if let Some(node) = headers.get("x-node-id").and_then(|v| v.to_str().ok()) {
        state.store.transaction(|data|{
            data.set("peer_inbound", &format!("{}:{}",credential.project_id,credential.client_id),json!({"project_id":credential.project_id,"client_id":credential.client_id,"node_id":node,"remote_address":peer.as_ref().map(|p|p.0.0.to_string())}));
            Ok(())
        }).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    let (tx, rx) = mpsc::channel(32);
    state.clients.lock().await.insert(
        (credential.project_id, credential.client_id.clone()),
        ClientConnection {
            sender: tx,
            role: credential.role.clone(),
            node_id: headers
                .get("x-node-id")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned),
        },
    );
    let ready = event(
        Uuid::nil(),
        "ready",
        json!({"client_id":credential.client_id,"project_id":credential.project_id,"node_id":state.node_id}),
    );
    let check_state = state.clone();
    let blocked_key = format!("{}:{}", credential.project_id, credential.client_id);
    let until_blocked = async move {
        loop {
            if check_state
                .store
                .get("peer_blocks", &blocked_key)
                .await
                .is_some_and(|r| r["disabled"] == true)
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        }
    };
    let stream = stream::once(async move { Ok(sse("ready", &ready)) })
        .chain(ReceiverStream::new(rx).map(|e| Ok(sse("command", &e))))
        .take_until(until_blocked);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
pub(crate) async fn client_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<ClientEvent>,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    let credential = auth_client(&state, &headers)
        .await
        .map_err(|status| (status, Json(json!({"error":"unauthorized"}))))?;
    accept_client_event(state, credential, input).await
}
pub(crate) async fn accept_client_event(
    state: AppState,
    credential: Credential,
    input: ClientEvent,
) -> Result<(StatusCode, Json<Value>), (StatusCode, Json<Value>)> {
    if conversation::stopped(&state, input.message_id).await {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error":"run interrupted"})),
        ));
    }
    let (events, project_id, assignee) = state
        .sessions
        .lock()
        .await
        .get(&input.session_id)
        .map(|s| {
            (
                s.events.clone(),
                s.project_id,
                s.requests.get(&input.message_id).cloned(),
            )
        })
        .ok_or((
            StatusCode::NOT_FOUND,
            Json(json!({"error":"unknown session"})),
        ))?;
    if credential.project_id != project_id || assignee.as_deref() != Some(&credential.client_id) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(json!({"error":"project isolation violation"})),
        ));
    }
    if !matches!(
        input.kind.as_str(),
        "agent.delta"
            | "agent.message"
            | "agent.done"
            | "agent.error"
            | "agent.tool.started"
            | "agent.tool.finished"
            | "agent.context"
    ) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({"error":"unsupported event type"})),
        ));
    }
    let event = event(
        input.session_id,
        input.kind,
        json!({"message_id":input.message_id,"content":input.content,"error":input.error,"error_code":input.error_code}),
    );
    if !persist_event(
        &state,
        project_id,
        &format!("session:{}", input.session_id),
        &event,
    )
    .await
    {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({"error":"run interrupted or persistence failed"})),
        ));
    }
    if matches!(event.kind.as_str(), "agent.done" | "agent.error") {
        if let Some(session) = state.sessions.lock().await.get_mut(&input.session_id) {
            session.requests.remove(&input.message_id);
        }
    }
    let _ = events.send(event.clone());
    Ok((StatusCode::ACCEPTED, Json(json!({"ok":true}))))
}
