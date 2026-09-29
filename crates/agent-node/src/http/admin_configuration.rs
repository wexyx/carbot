use crate::{AppState, management::Manager};
use axum::{Extension, Json, Router, http::StatusCode, routing::get};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc};

// Human-only bootstrap settings. Deliberately absent from the model's tool registry.
pub(crate) fn routes(manager: Arc<Manager>) -> Router<AppState> {
    Router::new()
        .route("/v1/admin-agent/configuration", get(read).put(update))
        .layer(Extension(manager))
}
async fn read(
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let settings = m.configuration().await.map_err(crate::control::api_error)?;
    Ok(Json(
        json!({"configuration":settings.public_view(),"active":m.status().await}),
    ))
}
async fn update(
    Extension(m): Extension<Arc<Manager>>,
    Json(values): Json<BTreeMap<String, String>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let mut settings = m.configuration().await.map_err(crate::control::api_error)?;
    settings.update(values).map_err(crate::control::api_error)?;
    m.reconfigure(settings)
        .await
        .map_err(|message| (StatusCode::CONFLICT, Json(json!({"error":message}))))?;
    Ok(Json(json!({"status":"saved","active":m.status().await})))
}
