use crate::{AppState, control, management::Manager};
use axum::{Extension, Json, Router, routing::get};
use serde_json::Value;
use std::sync::Arc;
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/permissions/allowlist", get(read).put(save))
        .layer(Extension(manager))
}
async fn read(Extension(m): Extension<Arc<Manager>>) -> Json<Value> {
    Json(m.core().command_allowlist().await)
}
async fn save(
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.core()
        .set_command_allowlist(input)
        .await
        .map(Json)
        .map_err(control::api_error)
}
