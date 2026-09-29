use crate::{AppState, control, management::Manager};
use axum::{Extension, Json, Router, extract::Path, routing::get};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/repl/{project}/skills/{scope}", get(list).put(save))
        .layer(Extension(manager))
}
async fn list(
    Path((p, scope)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "skills.list", json!({"scope":scope}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn save(
    Path((p, scope)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "skills.save", json!({"scope":scope,"body":input}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
