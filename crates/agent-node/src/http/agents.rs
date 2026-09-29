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
fn reply(result: Result<Value, String>) -> Reply {
    result.map(Json).map_err(control::api_error)
}
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/repl/{p}/agents", get(list).put(save))
        .route("/v1/repl/{p}/agents/candidates", get(candidates))
        .route("/v1/repl/{p}/agent-tests", post(test_chat))
        .route("/v1/repl/{p}/agents/{id}", axum::routing::delete(remove))
        .route(
            "/v1/repl/{p}/virtual-agents",
            post(save_virtual).put(save_virtual),
        )
        .route(
            "/v1/repl/{p}/agents/{id}/permissions",
            get(permissions).put(set_permissions),
        )
        .route("/v1/repl/{p}/agents/{id}/start", post(start))
        .route("/v1/repl/{p}/agents/{id}/stop", post(stop))
        .layer(Extension(manager))
}
async fn list(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.core().agent_directory(p).await)
}
async fn save(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(m.workbench(p, "agents.save", v).await)
}
async fn start(
    Path((p, id)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    reply(m.workbench(p, "agents.start", json!({"id":id})).await)
}
async fn stop(Path((p, id)): Path<(Uuid, String)>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.workbench(p, "agents.stop", json!({"id":id})).await)
}

async fn save_virtual(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(m.workbench(p, "agents.virtual", v).await)
}

async fn candidates(Path(p): Path<Uuid>, Extension(m): Extension<Arc<Manager>>) -> Reply {
    reply(m.workbench(p, "agents.list", json!({})).await)
}
async fn test_chat(
    Path(p): Path<Uuid>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(m.workbench(p, "agents.test", v).await)
}
async fn remove(
    Path((p, id)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    let expected = v["expected_version"]
        .as_u64()
        .ok_or_else(|| control::api_error("expected_version required".into()))?;
    reply(
        m.workbench(
            p,
            "agents.delete",
            json!({"id":id,"expected_version":expected}),
        )
        .await,
    )
}

async fn permissions(
    Path((p, id)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
) -> Reply {
    reply(m.core().permissions(p, &id).await)
}
async fn set_permissions(
    Path((p, id)): Path<(Uuid, String)>,
    Extension(m): Extension<Arc<Manager>>,
    Json(v): Json<Value>,
) -> Reply {
    reply(m.core().set_permissions(p, &id, v).await)
}
