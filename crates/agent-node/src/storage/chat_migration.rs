use super::{ChatLog, Data};
use serde_json::{Value, json};
use std::{collections::HashMap, path::Path};
use uuid::Uuid;

// Retry-safe one-time export. The old snapshot remains authoritative until every
// append succeeds and the caller atomically saves metadata without chat bodies.
pub(super) async fn migrate(dir: &Path, data: &mut Data, logs: &ChatLog) -> Result<bool, String> {
    let history = data.list("history");
    let sessions = data.list("management_sessions");
    if history.is_empty()
        && !sessions
            .iter()
            .any(|r| r["events"].as_array().is_some_and(|a| !a.is_empty()))
    {
        return Ok(false);
    }
    let backup = dir.join("state.before-chat-jsonl.json");
    if !backup.exists() {
        std::fs::copy(dir.join("state.json"), &backup).map_err(|e| e.to_string())?;
    }
    let mut buckets: HashMap<(Uuid, String), Vec<Value>> = HashMap::new();
    let mut history = history;
    history.sort_by_key(|r| r["seq"].as_u64().unwrap_or(0));
    for mut row in history {
        let p = Uuid::parse_str(
            row["project_id"]
                .as_str()
                .ok_or("legacy history missing project")?,
        )
        .map_err(|e| e.to_string())?;
        let channel = row["channel"]
            .as_str()
            .ok_or("legacy history missing channel")?
            .to_owned();
        let run = channel
            .strip_prefix("session:")
            .and_then(|s| data.get("runs", s));
        let chat = run
            .and_then(|r| r["group_id"].as_str())
            .map(|g| format!("group:{g}"))
            .unwrap_or(channel);
        row["legacy_seq"] = row["seq"].take();
        row.as_object_mut().unwrap().remove("seq");
        buckets.entry((p, chat)).or_default().push(row);
    }
    let mut sessions = sessions;
    sessions.sort_by_key(|s| (s["updated_at"].as_u64().unwrap_or(0), s["id"].to_string()));
    for row in &sessions {
        let p = Uuid::parse_str(
            row["project_id"]
                .as_str()
                .ok_or("legacy session missing project")?,
        )
        .map_err(|e| e.to_string())?;
        for mut event in row["events"].as_array().into_iter().flatten().cloned() {
            event["source_session"] = row["id"].clone();
            event["legacy_seq"] = event["seq"].take();
            event.as_object_mut().unwrap().remove("seq");
            buckets.entry((p, "admin".into())).or_default().push(event);
        }
    }
    for ((p, chat), events) in buckets {
        let saved = logs
            .read(p, chat.clone(), 0, u64::MAX, 1)
            .await?
            .last()
            .and_then(|e| e["seq"].as_u64())
            .unwrap_or(0) as usize;
        if saved > events.len() {
            return Err("legacy export cursor exceeds history; refusing migration".into());
        }
        for chunk in events[saved..].chunks(128) {
            logs.append(p, chat.clone(), chunk.to_vec()).await?;
        }
    }
    data.collections.remove("history");
    for mut row in sessions {
        let p = Uuid::parse_str(row["project_id"].as_str().unwrap()).unwrap();
        row.as_object_mut().unwrap().remove("events");
        row["id"] = json!(p);
        data.set("management_sessions", &p.to_string(), row);
    }
    // Old metadata aliases are unnecessary once all events carry their source ID.
    if let Some(rows) = data.collections.get_mut("management_sessions") {
        rows.retain(|_, r| r["id"] == r["project_id"]);
        for row in rows.values_mut() {
            row.as_object_mut().unwrap().remove("events");
        }
    }
    Ok(true)
}
