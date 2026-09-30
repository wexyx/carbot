use crate::{control, management::Manager};
use axum::{
    Extension, Json,
    extract::{Path, Query},
    http::StatusCode,
    response::sse::{Event, KeepAlive, Sse},
};
use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{convert::Infallible, sync::Arc};
use uuid::Uuid;
#[derive(Deserialize)]
pub(crate) struct Page {
    #[serde(default)]
    after: u64,
    #[serde(default)]
    before: Option<u64>,
    #[serde(default)]
    limit: Option<usize>,
}
async fn key(m: &Manager, p: Uuid, chat: &str) -> Result<String, String> {
    m.core().project(p).await?;
    if chat == "admin" {
        return Ok("admin".into());
    }
    m.core().state().policy_store.get(p, "group", chat).await?;
    Ok(format!("group:{chat}"))
}
pub(crate) async fn history(
    Path((p, chat)): Path<(Uuid, String)>,
    Query(page): Query<Page>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let key = key(&m, p, &chat).await.map_err(control::api_error)?;
    let logs = m.core().state().store.logs();
    let rows = logs
        .read(
            p,
            key.clone(),
            page.after,
            page.before.unwrap_or(u64::MAX),
            page.limit.unwrap_or(300).clamp(1, 1000),
        )
        .await
        .map_err(control::api_error)?;
    let files = logs.files(p, key).await.map_err(control::api_error)?;
    let runs = m.core().state().store.list("runs").await;
    let active = runs
        .iter()
        .find(|r| r["project_id"] == json!(p) && r["group_id"] == chat && r["status"] == "running");
    let events = rows.into_iter().map(|r| flatten(r)).collect::<Vec<_>>();
    Ok(Json(
        json!({"events":events,"files":files,"active_run":active.map(|r|r["id"].clone()),"has_more":events.first().and_then(|e|e["seq"].as_u64()).is_some_and(|s|s>1)}),
    ))
}
fn flatten(mut row: Value) -> Value {
    if row.get("payload").is_some() {
        let mut event = row["payload"].take();
        event["seq"] = row["seq"].clone();
        event["logged_at"] = row["logged_at"].clone();
        event
    } else {
        row
    }
}
pub(crate) async fn events(
    Path((p, chat)): Path<(Uuid, String)>,
    Query(page): Query<Page>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<
    Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>,
    (StatusCode, Json<Value>),
> {
    let key = key(&m, p, &chat).await.map_err(control::api_error)?;
    let log = m.core().state().store.logs().clone();
    let stream = futures_util::stream::unfold(
        (
            log,
            key,
            page.after,
            std::collections::VecDeque::new(),
            false,
        ),
        move |(log, key, mut seq, mut queue, failed)| async move {
            if failed {
                return None;
            }
            loop {
                if let Some(row) = queue.pop_front() {
                    let row: Value = row;
                    seq = row["seq"].as_u64().unwrap_or(seq);
                    return Some((
                        Ok(Event::default()
                            .id(seq.to_string())
                            .data(flatten(row).to_string())),
                        (log, key, seq, queue, false),
                    ));
                }
                // The live poll must not take a write barrier: it used to, which made a
                // streaming response pay an fsync several times a second and held up the
                // writer it was waiting on.
                match log.read_recent(p, key.clone(), seq, 1000).await {
                    Ok(rows) => queue = rows.into(),
                    Err(error) => {
                        return Some((
                            Ok(Event::default()
                                .data(json!({"type":"stream_error","message":error}).to_string())),
                            (log, key, seq, queue, true),
                        ));
                    }
                }
                if queue.is_empty() {
                    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                }
            }
        },
    );
    Ok(Sse::new(
        futures_util::stream::once(async { Ok(Event::default().comment("connected")) })
            .chain(stream),
    )
    .keep_alive(KeepAlive::default()))
}

#[derive(Deserialize)]
pub(crate) struct FilePage {
    #[serde(default)]
    offset: u64,
    from_line: Option<u64>,
    to_line: Option<u64>,
}
pub(crate) async fn file(
    Path((p, chat, name)): Path<(Uuid, String, String)>,
    Query(page): Query<FilePage>,
    Extension(m): Extension<Arc<Manager>>,
) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    let chat = key(&m, p, &chat).await.map_err(control::api_error)?;
    if page.from_line.is_some() || page.to_line.is_some() {
        if page.offset != 0 {
            return Err(control::api_error(
                "offset and line range cannot be combined".into(),
            ));
        }
        return m
            .core()
            .state()
            .store
            .logs()
            .read_lines(
                p,
                chat,
                Some(name),
                page.from_line.unwrap_or(1),
                page.to_line
                    .unwrap_or_else(|| page.from_line.unwrap_or(1).saturating_add(99)),
            )
            .await
            .map(Json)
            .map_err(control::api_error);
    }
    m.core()
        .state()
        .store
        .logs()
        .read_file(p, chat, name, page.offset)
        .await
        .map(Json)
        .map_err(control::api_error)
}
