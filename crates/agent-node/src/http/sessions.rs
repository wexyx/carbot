use crate::*;
pub async fn interrupt(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    conversation::interrupt_session(&state, id)
        .await
        .map(Json)
        .map_err(control::api_error)
}
pub(crate) async fn session_events(
    Path(id): Path<Uuid>,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, StatusCode> {
    let rx = state
        .sessions
        .lock()
        .await
        .get(&id)
        .map(|s| s.events.subscribe())
        .ok_or(StatusCode::NOT_FOUND)?;
    let stream = BroadcastStream::new(rx)
        .filter_map(|result| async move { result.ok().map(|e| Ok(sse(&e.kind, &e))) });
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keepalive"),
    ))
}
