//! Ordered, bounded background writes. Streaming producers never wait for fsync.
use super::chat_log::{ChatLog, Inner};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

type ResultRows = Result<Vec<Value>, String>;
struct Append {
    project: Uuid,
    chat: String,
    events: Vec<Value>,
    reply: Option<oneshot::Sender<ResultRows>>,
}
enum Message {
    Append(Append),
    Barrier(oneshot::Sender<Result<(), String>>),
}
pub(super) struct LogWriter {
    sender: mpsc::Sender<Message>,
    failure: Arc<Mutex<Option<String>>>,
}
impl LogWriter {
    pub(super) fn new(inner: Weak<Inner>) -> Self {
        let (sender, receiver) = mpsc::channel(512);
        let failure = Arc::new(Mutex::new(None));
        tokio::spawn(run(inner, receiver, failure.clone()));
        Self { sender, failure }
    }
    fn check(&self) -> Result<(), String> {
        match self.failure.lock().unwrap().as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
    pub(super) fn enqueue(
        &self,
        project: Uuid,
        chat: String,
        events: Vec<Value>,
    ) -> Result<(), String> {
        self.check()?;
        self.sender
            .try_send(Message::Append(Append {
                project,
                chat,
                events,
                reply: None,
            }))
            .map_err(|error| format!("chat log queue unavailable: {error}"))
    }
    pub(super) async fn append(
        &self,
        project: Uuid,
        chat: String,
        events: Vec<Value>,
    ) -> ResultRows {
        self.check()?;
        let (reply, result) = oneshot::channel();
        self.sender
            .send(Message::Append(Append {
                project,
                chat,
                events,
                reply: Some(reply),
            }))
            .await
            .map_err(|_| "chat log writer closed")?;
        result.await.map_err(|_| "chat log writer stopped")?
    }
    pub(super) async fn flush(&self) -> Result<(), String> {
        self.check()?;
        let (reply, result) = oneshot::channel();
        self.sender
            .send(Message::Barrier(reply))
            .await
            .map_err(|_| "chat log writer closed")?;
        result.await.map_err(|_| "chat log writer stopped")?
    }
}

async fn run(
    inner: Weak<Inner>,
    mut receiver: mpsc::Receiver<Message>,
    failure: Arc<Mutex<Option<String>>>,
) {
    while let Some(first) = receiver.recv().await {
        let mut batch = vec![first];
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(40);
        // Only a read barrier closes the batch early. An acknowledged append can ride
        // along with the deltas behind it: its oneshot is answered after the whole batch
        // is durable, so there is nothing to gain from paying an extra fsync for it.
        while batch.len() < 128 && !matches!(batch.last(), Some(Message::Barrier(_))) {
            match tokio::time::timeout_at(deadline, receiver.recv()).await {
                Ok(Some(message)) => batch.push(message),
                _ => break,
            }
        }
        let Some(inner) = inner.upgrade() else { break };
        let worker_failure = failure.clone();
        // All file operations run off the async executor. One writer preserves order.
        if let Err(error) =
            tokio::task::spawn_blocking(move || commit(ChatLog(inner), batch, &worker_failure))
                .await
        {
            let error = format!("chat log writer failed: {error}");
            eprintln!("{error}");
            *failure.lock().unwrap() = Some(error);
        }
    }
}

fn commit(log: ChatLog, batch: Vec<Message>, failure: &Mutex<Option<String>>) {
    let mut pending: Vec<Append> = Vec::new();
    for message in batch {
        match message {
            Message::Append(next) => pending.push(next),
            Message::Barrier(reply) => {
                commit_all(&log, &mut pending, failure);
                let result = failure.lock().unwrap().clone().map_or(Ok(()), Err);
                let _ = reply.send(result);
            }
        }
    }
    commit_all(&log, &mut pending, failure);
}

/// One commit per chat in the batch, in first-appearance order. A group run interleaves
/// two directories (the member's own session and the group chat), and splitting on every
/// alternation reduced batching to a single fsync per streamed token.
fn commit_all(log: &ChatLog, pending: &mut Vec<Append>, failure: &Mutex<Option<String>>) {
    if pending.is_empty() {
        return;
    }
    let mut order: Vec<(Uuid, String)> = Vec::new();
    let mut grouped: HashMap<(Uuid, String), Vec<Append>> = HashMap::new();
    for entry in pending.drain(..) {
        let key = (entry.project, entry.chat.clone());
        if !order.contains(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(entry);
    }
    for key in order {
        if let Some(mut chat) = grouped.remove(&key) {
            commit_chat(log, &mut chat, failure);
        }
    }
}
fn commit_chat(log: &ChatLog, pending: &mut Vec<Append>, failure: &Mutex<Option<String>>) {
    let Some(first) = pending.first() else { return };
    #[cfg(test)]
    log.0
        .commits
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let previous = failure.lock().unwrap().clone();
    let project = first.project;
    let chat = first.chat.clone();
    let counts: Vec<_> = pending.iter().map(|entry| entry.events.len()).collect();
    let events = pending
        .iter_mut()
        .flat_map(|entry| std::mem::take(&mut entry.events))
        .collect();
    let result = match previous {
        Some(error) => Err(error),
        None => log.append_sync(project, &chat, events),
    };
    if let Err(error) = &result {
        let mut stored = failure.lock().unwrap();
        if stored.is_none() {
            eprintln!("history persistence failed: {error}");
            *stored = Some(error.clone());
        }
    }
    let mut offset = 0;
    for (entry, count) in pending.drain(..).zip(counts) {
        if let Some(reply) = entry.reply {
            let rows = result
                .as_ref()
                .map(|rows| rows[offset..offset + count].to_vec())
                .map_err(Clone::clone);
            let _ = reply.send(rows);
        }
        offset += count;
    }
}
