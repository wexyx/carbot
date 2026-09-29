use crate::{AppState, control, management::Manager};
use axum::{
    Extension, Json, Router,
    extract::{Path, Query},
    routing::{get, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
type Reply = Result<Json<Value>, (axum::http::StatusCode, Json<Value>)>;
#[derive(Deserialize, Default)]
struct Target {
    #[serde(default)]
    agent: String,
    group: Option<String>,
}
pub(crate) fn routes(m: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route(
            "/v1/repl/{p}/capabilities/{scope}/{kind}",
            get(list).put(save),
        )
        .route(
            "/v1/repl/{p}/capabilities/{scope}/{kind}/bindings",
            put(bind),
        )
        .layer(Extension(m))
}
async fn list(
    Path((p, s, k)): Path<(Uuid, String, String)>,
    Query(q): Query<Target>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    m.workbench(
        p,
        "capabilities.list",
        json!({"scope":s,"kind":k,"agent":q.agent,"group":q.group}),
    )
    .await
    .map(Json)
    .map_err(control::api_error)
}
async fn save(
    Path((p, s, k)): Path<(Uuid, String, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    m.workbench(p, "capabilities.save", json!({"scope":s,"kind":k,"body":v}))
        .await
        .map(Json)
        .map_err(control::api_error)
}
async fn bind(
    Path((p, s, k)): Path<(Uuid, String, String)>,
    Query(q): Query<Target>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    m.workbench(
        p,
        "capabilities.bind",
        json!({"scope":s,"kind":k,"agent":q.agent,"group":q.group,"body":v}),
    )
    .await
    .map(Json)
    .map_err(control::api_error)
}
