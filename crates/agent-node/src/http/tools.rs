use crate::{AppState, control, management::Manager};
use axum::{
    Extension, Json, Router,
    extract::Path,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
pub(crate) fn routes(m: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route(
            "/v1/repl/{p}/tool-config/{scope}/{agent}",
            get(read).put(save),
        )
        .route(
            "/v1/repl/{p}/tool-config/{scope}/{agent}/tests",
            post(start_test),
        )
        .route(
            "/v1/repl/{p}/tool-tests/{id}",
            get(test_status).delete(cancel_test),
        )
        .layer(Extension(m))
}
async fn start_test(
    Path((p, s, a)): Path<(Uuid, String, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "tools.test", json!({"scope":s,"agent":a,"body":input}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn test_status(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "tools.test-status", json!({"id":id}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn cancel_test(
    Path((p, id)): Path<(Uuid, Uuid)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "tools.test-cancel", json!({"id":id}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn read(
    Path((p, s, a)): Path<(Uuid, String, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "tools.list", json!({"scope":s,"agent":a}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn save(
    Path((p, s, a)): Path<(Uuid, String, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(input): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, Json<Value>)> {
    m.workbench(p, "tools.save", json!({"scope":s,"agent":a,"body":input}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
