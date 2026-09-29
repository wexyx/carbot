use serde_json::{Value, json};
use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, watch};
use uuid::Uuid;

struct Entry {
    project: Uuid,
    value: Value,
    created: Instant,
    cancel: watch::Sender<bool>,
}
#[derive(Clone, Default)]
pub(super) struct TestRuns {
    entries: Arc<Mutex<HashMap<Uuid, Entry>>>,
}
impl TestRuns {
    pub(super) async fn start(
        &self,
        project: Uuid,
        tool: String,
        execute: impl Future<Output = Result<Value, String>> + Send + 'static,
    ) -> Result<Value, String> {
        let mut entries = self.entries.lock().await;
        entries.retain(|_, e| {
            e.value["status"] == "running" || e.created.elapsed() < Duration::from_secs(900)
        });
        if entries
            .values()
            .filter(|e| e.value["status"] == "running")
            .count()
            >= 4
        {
            return Err("too many active tool tests (maximum 4)".into());
        }
        if entries.len() >= 64 {
            let oldest = entries
                .iter()
                .filter(|(_, e)| e.value["status"] != "running")
                .min_by_key(|(_, e)| e.created)
                .map(|(id, _)| *id);
            if let Some(id) = oldest {
                entries.remove(&id);
            }
        }
        let id = Uuid::new_v4();
        let (cancel, mut receiver) = watch::channel(false);
        let value = json!({"id":id,"tool":tool,"status":"running","output":null,"error":null});
        entries.insert(
            id,
            Entry {
                project,
                value: value.clone(),
                created: Instant::now(),
                cancel,
            },
        );
        drop(entries);
        let entries = self.entries.clone();
        tokio::spawn(async move {
            let start = Instant::now();
            let result = tokio::select! {
                _=receiver.changed()=>Err("test cancelled".into()),
                result=tokio::time::timeout(Duration::from_secs(240),agent_runtime::workspace::with_approval_context(id,execute))=>result.unwrap_or_else(|_|Err("test timed out".into())),
            };
            if let Some(entry) = entries.lock().await.get_mut(&id) {
                entry.value["duration_ms"] = json!(start.elapsed().as_millis());
                match result {
                    Ok(output) => {
                        entry.value["status"] = json!("completed");
                        let text = output.to_string();
                        entry.value["output"] = if text.len() > 65536 {
                            let mut end = 65536;
                            while !text.is_char_boundary(end) {
                                end -= 1;
                            }
                            json!({"truncated":true,"preview":&text[..end]})
                        } else {
                            output
                        };
                    }
                    Err(error) => {
                        entry.value["status"] = json!(if *receiver.borrow() {
                            "cancelled"
                        } else {
                            "failed"
                        });
                        entry.value["error"] = json!(error);
                    }
                }
            }
        });
        Ok(value)
    }
    pub(super) async fn read(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        let entries = self.entries.lock().await;
        let row = entries
            .get(&id)
            .filter(|e| e.project == project)
            .ok_or("unknown tool test")?;
        Ok(row.value.clone())
    }
    pub(super) async fn cancel(&self, project: Uuid, id: Uuid) -> Result<Value, String> {
        let entries = self.entries.lock().await;
        let row = entries
            .get(&id)
            .filter(|e| e.project == project)
            .ok_or("unknown tool test")?;
        let _ = row.cancel.send(true);
        Ok(json!({"id":id,"status":"cancelling"}))
    }
    pub(super) async fn stop(&self) {
        for row in self.entries.lock().await.values() {
            let _ = row.cancel.send(true);
        }
    }
}
