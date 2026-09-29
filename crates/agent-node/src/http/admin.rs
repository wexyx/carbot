use crate::management::Manager;
use crate::{AppState, control};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::sse::{Event, KeepAlive, Sse},
    routing::{get, post},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use std::{collections::VecDeque, convert::Infallible, sync::Arc};
use uuid::Uuid;
type Reply = Result<Json<Value>, (StatusCode, Json<Value>)>;
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/admin-agent", get(status))
        .route("/v1/admin-agent/{project}/approvals", get(approvals))
        .route("/v1/admin-agent/{project}/approvals/{id}", post(decide))
        .route(
            "/v1/admin-agent/{project}/sessions",
            get(sessions).post(create_session),
        )
        .route("/v1/admin-agent/{project}/sessions/{id}", get(history))
        .route(
            "/v1/admin-agent/{project}/sessions/{id}/new",
            post(reset_context),
        )
        .route(
            "/v1/admin-agent/{project}/sessions/{id}/messages",
            post(message),
        )
        .route(
            "/v1/admin-agent/{project}/sessions/{id}/interrupt",
            post(interrupt),
        )
        .route(
            "/v1/admin-agent/{project}/sessions/{id}/events",
            get(events),
        )
        .layer(Extension(manager))
}
fn reply(value: Result<Value, String>) -> Reply {
    value.map(Json).map_err(control::api_error)
}
async fn status(Extension(m): Extension<Arc<Manager>>) -> Json<Value> {
    Json(m.status().await)
}
async fn sessions(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.sessions(p).await)
}
async fn create_session(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.create_session(p).await)
}
async fn reset_context(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    reply(m.reset_context(p, id).await)
}
async fn history(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    reply(m.history(p, id).await)
}
async fn message(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(
        m.message(p, id, v["content"].as_str().unwrap_or_default().into())
            .await,
    )
}
async fn interrupt(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    reply(m.interrupt(p, id).await)
}
#[derive(Deserialize, Default)]
struct Cursor {
    #[serde(default)]
    after: u64,
}
async fn events(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Query(cursor): Query<Cursor>,
    headers: HeaderMap,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let receiver = m.subscribe();
    let history = m.history(p, id).await.map_err(control::api_error)?;
    let after = headers
        .get("last-event-id")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok())
        .unwrap_or(cursor.after);
    let queue: VecDeque<Value> = history["events"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|e| e["seq"].as_u64().unwrap_or(0) > after)
        .cloned()
        .collect();
    let stream = futures_util::stream::unfold(
        (m, after, queue, receiver),
        move |(m, mut seq, mut queue, mut receiver)| async move {
            loop {
                if let Some(item) = queue.pop_front() {
                    let next = item["seq"].as_u64().unwrap_or(0);
                    if next <= seq {
                        continue;
                    }
                    seq = next;
                    let event = Event::default()
                        .id(seq.to_string())
                        .event("management")
                        .data(item.to_string());
                    return Some((Ok(event), (m, seq, queue, receiver)));
                }
                match receiver.recv().await {
                    Ok(item) if item.session == id => queue.push_back(item.event),
                    Ok(_) => {}
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let history = m.history(p, id).await.ok()?;
                        queue.extend(
                            history["events"]
                                .as_array()
                                .into_iter()
                                .flatten()
                                .filter(|e| e["seq"].as_u64().unwrap_or(0) > seq)
                                .cloned(),
                        );
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => return None,
                }
            }
        },
    );
    let stream = futures_util::stream::once(async {
        Ok::<_, Infallible>(Event::default().comment("connected"))
    })
    .chain(stream);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn approvals(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.core().approvals(p).await)
}
async fn decide(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Decision>,
) -> Reply {
    reply(m.core().decide(p, id, v.allow).await)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Decision {
    allow: bool,
}
