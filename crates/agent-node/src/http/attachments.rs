use crate::AppState;
use agent_runtime::attachments::{AttachmentStore, MAX_FILE_BYTES};
use axum::{
    Json, Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::json;

pub(crate) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/attachments", post(upload))
        .route("/v1/attachments/{id}/content", get(download))
        .layer(DefaultBodyLimit::max(MAX_FILE_BYTES))
}
#[derive(Deserialize)]
struct Upload {
    name: String,
}
async fn upload(
    Query(query): Query<Upload>,
    body: Bytes,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let result =
        tokio::task::spawn_blocking(move || AttachmentStore::default().upload(&query.name, &body))
            .await
            .map_err(|e| crate::control::api_error(e.to_string()))?
            .map_err(crate::control::api_error)?;
    Ok(Json(
        json!({"attachment":result,"reference":result.reference()}),
    ))
}
#[derive(Deserialize)]
struct Download {
    #[serde(default)]
    preview: bool,
}
async fn download(Path(id): Path<String>, Query(query): Query<Download>) -> Response {
    match tokio::task::spawn_blocking(move || AttachmentStore::default().read(&id)).await {
        Ok(Ok((metadata, bytes, _)))
            if query.preview && metadata.media_type().starts_with("image/") =>
        {
            (
                [
                    (header::CONTENT_TYPE, metadata.media_type().to_owned()),
                    (header::CACHE_CONTROL, "no-store".into()),
                    (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
                    (
                        header::CONTENT_SECURITY_POLICY,
                        "default-src 'none'; sandbox".into(),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Ok(Ok(_)) if query.preview => StatusCode::UNSUPPORTED_MEDIA_TYPE.into_response(),
        Ok(Ok((_metadata, bytes, _))) => (
            [
                (header::CONTENT_TYPE, "application/octet-stream"),
                (header::CONTENT_DISPOSITION, "attachment"),
                (header::CACHE_CONTROL, "no-store"),
                (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            ],
            bytes,
        )
            .into_response(),
        _ => (
            StatusCode::NOT_FOUND,
            Json(json!({"error":"附件不存在或已变化，请重新添加"})),
        )
            .into_response(),
    }
}
