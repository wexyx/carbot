use crate::AppState;
use serde_json::Value;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{Mutex, Semaphore};
use uuid::Uuid;

struct Entry {
    rows: Vec<Value>,
    updated: Instant,
    loading: bool,
}
pub(super) struct RemoteDirectory {
    entries: Mutex<HashMap<(Uuid, String), Entry>>,
    permits: Semaphore,
}
impl RemoteDirectory {
    pub(super) fn new() -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::new()),
            permits: Semaphore::new(8),
        })
    }
    pub(super) async fn snapshot(
        self: &Arc<Self>,
        state: &AppState,
        project: Uuid,
        id: &str,
    ) -> Vec<Value> {
        let key = (project, id.to_owned());
        let mut entries = self.entries.lock().await;
        let entry = entries.entry(key.clone()).or_insert_with(|| Entry {
            rows: vec![],
            updated: Instant::now() - Duration::from_secs(10),
            loading: false,
        });
        let rows = entry.rows.clone();
        if !entry.loading && entry.updated.elapsed() >= Duration::from_secs(2) {
            entry.loading = true;
            let this = self.clone();
            let state = state.clone();
            tokio::spawn(async move {
                let _permit = this.permits.acquire().await.expect("directory semaphore");
                let result = super::remote_agents::catalog(&state, project, &key.1).await;
                let mut entries = this.entries.lock().await;
                if let Some(entry) = entries.get_mut(&key) {
                    entry.rows = result.unwrap_or_default();
                    entry.updated = Instant::now();
                    entry.loading = false;
                }
            });
        }
        rows
    }
}
