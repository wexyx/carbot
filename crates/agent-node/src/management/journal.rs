use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::broadcast;
use uuid::Uuid;

#[derive(Clone)]
pub(crate) struct LiveEvent {
    pub(crate) session: Uuid,
    pub(crate) event: Value,
}
#[derive(Clone)]
pub(super) struct Journal {
    live: Arc<Mutex<HashMap<Uuid, LiveSession>>>,
    events: broadcast::Sender<LiveEvent>,
}
struct LiveSession {
    row: Value,
    bytes: usize,
    seq: u64,
}
impl Default for Journal {
    fn default() -> Self {
        Self {
            live: Default::default(),
            events: broadcast::channel(2048).0,
        }
    }
}
impl Journal {
    pub(super) fn begin(&self, id: Uuid, mut row: Value) {
        let last = row["events"].as_array().and_then(|a| a.last()).cloned();
        let seq = last.as_ref().and_then(|e| e["seq"].as_u64()).unwrap_or(0);
        row["events"] = json!([]);
        let bytes = 0;
        self.live
            .lock()
            .unwrap()
            .insert(id, LiveSession { row, bytes, seq });
        if let Some(event) = last {
            let _ = self.events.send(LiveEvent { session: id, event });
        }
    }
    pub(super) fn history(&self, id: Uuid) -> Option<Value> {
        self.live.lock().unwrap().get(&id).map(|s| s.row.clone())
    }
    pub(super) fn emit(&self, id: Uuid, mut event: Value) -> Result<Value, String> {
        let mut live = self.live.lock().unwrap();
        let session = live.get_mut(&id).ok_or("unknown live session")?;
        let events = session.row["events"]
            .as_array_mut()
            .ok_or("invalid live session")?;
        let size = event.to_string().len() + 24;
        if events.len() >= 4094 || session.bytes + size > 2 * 1024 * 1024 - 8192 {
            return Err("management history limit reached; create a new session".into());
        }
        session.seq += 1;
        event["seq"] = json!(session.seq);
        event["logged_at"] = json!(crate::storage::now());
        events.push(event.clone());
        session.bytes += size;
        drop(live);
        let _ = self.events.send(LiveEvent {
            session: id,
            event: event.clone(),
        });
        Ok(event)
    }
    pub(super) fn committed(&self, id: Uuid, seq: u64) {
        if let Some(session) = self.live.lock().unwrap().get_mut(&id) {
            if let Some(events) = session.row["events"].as_array_mut() {
                events.retain(|e| e["seq"].as_u64().unwrap_or(0) > seq);
                session.bytes = events.iter().map(|e| e.to_string().len() + 24).sum();
            }
        }
    }
    pub(super) fn finish(&self, id: Uuid, event: Value) {
        self.live.lock().unwrap().remove(&id);
        let _ = self.events.send(LiveEvent { session: id, event });
    }
    pub(super) fn failed(&self, id: Uuid, message: String) {
        if let Some(session) = self.live.lock().unwrap().get_mut(&id) {
            let row = &mut session.row;
            row["status"] = json!("failed");
            row["persistence_failed"] = json!(true);
            if let Some(events) = row["events"].as_array_mut() {
                let event = json!({"seq":session.seq+1,"type":"failed","message":message.chars().take(2000).collect::<String>()});
                events.push(event.clone());
                let _ = self.events.send(LiveEvent { session: id, event });
            }
        }
    }
    pub(super) fn subscribe(&self) -> broadcast::Receiver<LiveEvent> {
        self.events.subscribe()
    }
}
