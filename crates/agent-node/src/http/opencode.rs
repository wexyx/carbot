use crate::{AppState, control, management::Manager};
use axum::{Extension, Json, Router, extract::Query, http::StatusCode, routing::get};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;

// Read-only host introspection that backs the OpenCode model picker. It starts
// no Agent session and only asks the local CLI which models it can route to.
//
// Which runner happens to be saved is irrelevant here: the picker is reachable
// precisely while the operator is choosing one, and a per-Agent override may
// name its own launcher. So the endpoint asks the CLI directly and lets the
// caller pass the launcher it actually intends to use.
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/opencode/models", get(models))
        .layer(Extension(manager))
}
#[derive(Deserialize, Default)]
struct Launcher {
    /// Only the launcher is honoured. Model and approval flags would change what
    /// a session does, and discovery must not be able to influence either.
    bin: Option<String>,
}
async fn models(
    Query(q): Query<Launcher>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let settings = m.configuration().await.map_err(control::api_error)?;
    let config = settings
        .opencode(q.bin.as_deref())
        .map_err(control::api_error)?;
    let catalog = agent_runtime::config::opencode_models(&config)
        .await
        .map_err(|message| (StatusCode::BAD_GATEWAY, Json(json!({"error":message}))))?;
    Ok(Json(json!({
        // An empty catalog means the local CLI is reachable but has no signed-in
        // account, which is a different remedy from a failing CLI.
        "signed_in": !catalog.models.is_empty(),
        "priced": catalog.source == agent_runtime::config::OpenCodeCatalogSource::Catalog,
        "source": catalog.source.as_str(),
        "models": catalog.models,
    })))
}
