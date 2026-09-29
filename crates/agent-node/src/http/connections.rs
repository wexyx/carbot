use crate::{AppState, control, management::Manager};
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
type Reply = Result<Json<Value>, (axum::http::StatusCode, Json<Value>)>;
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/repl/{p}/peers", get(list).put(configure))
        .route("/v1/repl/{p}/peers/mount", post(connect))
        .layer(Extension(manager))
}
async fn list(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    m.workbench(p, "connections.list", json!({}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn connect(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Reply {
    m.workbench(p, "connections.connect", input)
        .await
        .map(Json)
        .map_err(control::api_error)
}

async fn configure(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Reply {
    m.workbench(p, "connections.configure", input)
        .await
        .map(Json)
        .map_err(control::api_error)
}
