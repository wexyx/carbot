//! Hourly JSONL files. Memory contains cursors, not conversation snapshots.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex, OnceLock},
};
use uuid::Uuid;
#[derive(Clone)]
pub(crate) struct ChatLog(pub(super) Arc<Inner>);
/// How many recent events stay in memory per chat. A live stream asks for the newest
/// few, so anything older than this window is a history read that has to touch disk.
const TAIL_CAPACITY: usize = 4096;
/// Recent events per chat, kept so the streaming path never waits on the disk.
///
/// Only events that are already durable are recorded, so dropping the oldest one when
/// the window is full can never lose something a reader has not seen on disk yet.
#[derive(Default)]
struct Tail {
    events: std::collections::VecDeque<(u64, Value)>,
}
impl Tail {
    fn push(&mut self, seq: u64, event: Value) {
        self.events.push_back((seq, event));
        while self.events.len() > TAIL_CAPACITY {
            self.events.pop_front();
        }
    }
    /// Events after `after`, or `None` when the window cannot answer the whole request.
    fn after(&self, after: u64, before: u64, limit: usize) -> Option<Vec<Value>> {
        let oldest = self.events.front()?.0;
        // A request reaching past the window cannot be answered from memory alone.
        if after > 0 && after + 1 < oldest {
            return None;
        }
        Some(
            self.events
                .iter()
                .filter(|(seq, _)| *seq > after && *seq < before)
                .map(|(_, event)| event.clone())
                .take(limit)
                .collect(),
        )
    }
}
pub(super) struct Inner {
    writer: OnceLock<super::log_writer::LogWriter>,
    root: PathBuf,
    cursors: Mutex<HashMap<PathBuf, u64>>,
    line_indexes: Mutex<HashMap<PathBuf, super::log_line_index::LineIndex>>,
    tails: Mutex<HashMap<PathBuf, Tail>>,
    /// Durable commit count, so tests can prove batching instead of assuming it.
    #[cfg(test)]
    pub(super) commits: std::sync::atomic::AtomicUsize,
    _temporary: Option<tempfile::TempDir>,
}
impl ChatLog {
    pub(crate) fn open(root: PathBuf) -> Self {
        Self(Arc::new(Inner {
            writer: OnceLock::new(),
            root,
            cursors: Mutex::new(HashMap::new()),
            line_indexes: Mutex::new(HashMap::new()),
            tails: Mutex::new(HashMap::new()),
            #[cfg(test)]
            commits: Default::default(),
            _temporary: None,
        }))
    }
    #[cfg(test)]
    pub(crate) fn temporary() -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self(Arc::new(Inner {
            writer: OnceLock::new(),
            root: dir.path().into(),
            cursors: Mutex::new(HashMap::new()),
            line_indexes: Mutex::new(HashMap::new()),
            tails: Mutex::new(HashMap::new()),
            commits: Default::default(),
            _temporary: Some(dir),
        }))
    }
    fn directory(&self, project: Uuid, chat: &str) -> PathBuf {
        // Runs per append and per read; a format! per byte allocated ~45 times each.
        use std::fmt::Write;
        let mut key = String::with_capacity(chat.len() * 2);
        for byte in chat.as_bytes() {
            let _ = write!(key, "{byte:02x}");
        }
        self.0.root.join(project.to_string()).join(key)
    }
    pub(crate) async fn append(
        &self,
        project: Uuid,
        chat: String,
        events: Vec<Value>,
    ) -> Result<Vec<Value>, String> {
        self.writer().append(project, chat, events).await
    }
    fn writer(&self) -> &super::log_writer::LogWriter {
        self.0
            .writer
            .get_or_init(|| super::log_writer::LogWriter::new(Arc::downgrade(&self.0)))
    }
    pub(crate) fn enqueue(
        &self,
        project: Uuid,
        chat: String,
        events: Vec<Value>,
    ) -> Result<(), String> {
        self.writer().enqueue(project, chat, events)
    }
    pub(crate) async fn flush(&self) -> Result<(), String> {
        if let Some(writer) = self.0.writer.get() {
            writer.flush().await
        } else {
            Ok(())
        }
    }
    pub(super) fn append_sync(
        &self,
        project: Uuid,
        chat: &str,
        mut events: Vec<Value>,
    ) -> Result<Vec<Value>, String> {
        let dir = self.directory(project, chat);
        let mut cursors = self
            .0
            .cursors
            .lock()
            .map_err(|_| "chat log lock poisoned")?;
        let known = cursors.contains_key(&dir);
        let mut seq = match cursors.get(&dir) {
            Some(s) => *s,
            None => scan(&dir, 0, u64::MAX, 1)?
                .last()
                .and_then(|e| e["seq"].as_u64())
                .unwrap_or(0),
        };
        let now = super::now();
        let mut bytes = Vec::new();
        for event in &mut events {
            seq = seq.checked_add(1).ok_or("chat sequence overflow")?;
            if event.get("seq").is_some() && event["seq"] != json!(seq) {
                return Err("chat event sequence conflict".into());
            }
            event["seq"] = json!(seq);
            if event.get("logged_at").is_none() {
                event["logged_at"] = json!(now);
            }
            serde_json::to_writer(&mut bytes, event).map_err(|e| e.to_string())?;
            bytes.push(b'\n');
        }
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        // A cursor only exists once this directory was created and committed, so the
        // mkdirat and the stat below are redundant on every later append.
        let path = dir.join(format!("hour-{:012}.jsonl", now / 3600));
        let is_new_file = !path.exists();
        if !known {
            builder.create(&dir).map_err(|e| e.to_string())?;
        }
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|e| e.to_string())?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_data()) {
            if file.set_len(length).and_then(|_| file.sync_data()).is_err() {
                cursors.remove(&dir);
            }
            // Drop the undurable events, so a later read cannot serve from memory
            // something the log never accepted.
            if let Ok(mut tails) = self.0.tails.lock() {
                tails.remove(&dir);
            }
            return Err(format!("append chat log: {error}"));
        }
        cursors.insert(dir.clone(), seq);
        // The events are sequenced now, so a reader can be served from memory before
        // the bytes are durable; the watermark moves only once the write succeeded.
        if let Ok(mut tails) = self.0.tails.lock() {
            let tail = tails.entry(dir.clone()).or_default();
            for event in &events {
                if let Some(seq) = event["seq"].as_u64() {
                    tail.push(seq, event.clone());
                }
            }
        }
        if is_new_file {
            File::open(&dir)
                .and_then(|d| d.sync_all())
                .map_err(|e| e.to_string())?;
        }
        Ok(events)
    }
    pub(crate) async fn read(
        &self,
        project: Uuid,
        chat: String,
        after: u64,
        before: u64,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        self.flush().await?;
        let log = self.clone();
        tokio::task::spawn_blocking(move || {
            let mut guard = log.0.cursors.lock().map_err(|_| "chat log lock poisoned")?;
            let dir = log.directory(project, &chat);
            if after > 0 && guard.get(&dir).is_some_and(|last| *last <= after) {
                return Ok(vec![]);
            }
            let rows = scan(&dir, after, before, limit.clamp(1, 10000))?;
            if before == u64::MAX && after == 0 {
                if let Some(last) = rows.last().and_then(|e| e["seq"].as_u64()) {
                    guard.insert(dir, last);
                }
            }
            Ok(rows)
        })
        .await
        .map_err(|e| e.to_string())?
    }
    /// Read recent events without first waiting on the writer.
    ///
    /// The streaming endpoint polls this on a timer. Calling [`Self::read`] instead made
    /// every poll take a write barrier, so a live stream paid an fsync roughly six times
    /// a second and the writer's own commit queue behind it. Events reach this reader up
    /// to one batching window later, which is the trade for not stalling the writer.
    pub(crate) async fn read_recent(
        &self,
        project: Uuid,
        chat: String,
        after: u64,
        limit: usize,
    ) -> Result<Vec<Value>, String> {
        let dir = self.directory(project, &chat);
        let limit = limit.clamp(1, 10000);
        if let Ok(tails) = self.0.tails.lock()
            && let Some(rows) = tails
                .get(&dir)
                .and_then(|tail| tail.after(after, u64::MAX, limit))
        {
            return Ok(rows);
        }
        // The window did not cover this request, so it is a history read and has to be
        // ordered against the writer like any other.
        self.read(project, chat, after, u64::MAX, limit).await
    }

    pub(crate) async fn read_lines(
        &self,
        project: Uuid,
        chat: String,
        name: Option<String>,
        from: u64,
        to: u64,
    ) -> Result<Value, String> {
        self.flush().await?;
        let log = self.clone();
        tokio::task::spawn_blocking(move || {
            let directory = log.directory(project, &chat);
            let mut indexes = log
                .0
                .line_indexes
                .lock()
                .map_err(|_| "log line index poisoned")?;
            if let Some(name) = name {
                let file = super::log_line_index::LineIndex::open(&directory, &name)?;
                cached_index(&mut indexes, directory.join(&name)).read(file, &name, from, to)
            } else {
                let mut result = Vec::new();
                for path in files(&directory)? {
                    let name = path.file_name().unwrap().to_string_lossy().to_string();
                    let mut file = super::log_line_index::LineIndex::open(&directory, &name)?;
                    let count = cached_index(&mut indexes, path).refresh(&mut file)?;
                    result.push(json!({"file":name,"from_line":1,"to_line":count}));
                }
                Ok(json!(result))
            }
        })
        .await
        .map_err(|e| e.to_string())?
    }
    pub(crate) async fn read_file(
        &self,
        project: Uuid,
        chat: String,
        name: String,
        offset: u64,
    ) -> Result<Value, String> {
        self.flush().await?;
        let directory = self.directory(project, &chat);
        tokio::task::spawn_blocking(move || super::log_file_reader::read(&directory, &name, offset))
            .await
            .map_err(|e| e.to_string())?
    }
    pub(crate) async fn files(&self, project: Uuid, chat: String) -> Result<Vec<Value>, String> {
        self.flush().await?;
        let log = self.clone();
        tokio::task::spawn_blocking(move || {
            files(&log.directory(project, &chat))?
                .into_iter()
                .map(|p| {
                    let size = p.metadata().map_err(|e| e.to_string())?.len();
                    Ok(json!({"name":p.file_name().unwrap().to_string_lossy(),"bytes":size}))
                })
                .collect()
        })
        .await
        .map_err(|e| e.to_string())?
    }
}
fn cached_index(
    indexes: &mut HashMap<PathBuf, super::log_line_index::LineIndex>,
    path: PathBuf,
) -> &mut super::log_line_index::LineIndex {
    if !indexes.contains_key(&path) && indexes.len() >= 16 {
        if let Some(key) = indexes.keys().next().cloned() {
            indexes.remove(&key);
        }
    }
    indexes.entry(path).or_default()
}
fn files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = match std::fs::read_dir(dir) {
        Ok(v) => v,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.to_string()),
    };
    let mut paths = vec![];
    for e in entries {
        let p = e.map_err(|e| e.to_string())?.path();
        if p.extension().is_some_and(|e| e == "jsonl") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}
fn scan(dir: &Path, after: u64, before: u64, limit: usize) -> Result<Vec<Value>, String> {
    if after == 0 {
        return super::log_tail::read(files(dir)?, before, limit);
    }
    let mut result = std::collections::VecDeque::new();
    for path in files(dir)? {
        let mut reader = BufReader::new(File::open(&path).map_err(|e| e.to_string())?);
        let mut line = String::new();
        loop {
            line.clear();
            if reader.read_line(&mut line).map_err(|e| e.to_string())? == 0 {
                break;
            }
            if !line.ends_with('\n') {
                return Err(format!(
                    "incomplete chat log tail: {}; refusing to overwrite",
                    path.display()
                ));
            }
            let event: Value = serde_json::from_str(&line)
                .map_err(|e| format!("invalid chat log {}: {e}", path.display()))?;
            let seq = event["seq"].as_u64().ok_or("chat event missing seq")?;
            if seq > after && seq < before {
                result.push_back(event);
                if after > 0 && result.len() >= limit {
                    return Ok(result.into());
                }
                if result.len() > limit {
                    result.pop_front();
                }
            }
        }
    }
    Ok(result.into())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn streaming_queue_returns_while_disk_is_locked_and_reads_drain_in_order() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        // The producer can enqueue even when the file writer cannot acquire its lock.
        {
            let _disk = log.0.cursors.lock().unwrap();
            for n in 0..50 {
                log.enqueue(project, "stream".into(), vec![json!({"n":n})])
                    .unwrap();
            }
        }
        let terminal = log
            .append(project, "stream".into(), vec![json!({"type":"done"})])
            .await
            .unwrap();
        assert_eq!(terminal[0]["seq"], 51);
        let rows = log
            .read(project, "stream".into(), 0, u64::MAX, 100)
            .await
            .unwrap();
        assert_eq!(rows.len(), 51);
        for (n, row) in rows[..50].iter().enumerate() {
            assert_eq!(row["n"], n);
            assert_eq!(row["seq"], n + 1);
        }
        log.enqueue(project, "stream".into(), vec![json!({"type":"after"})])
            .unwrap();
        // History readers include queued content, not only previously committed content.
        assert_eq!(
            log.read(project, "stream".into(), 0, u64::MAX, 1)
                .await
                .unwrap()[0]["seq"],
            52
        );
    }
    /// A group run interleaves two chat directories (the member's own session and the
    /// group chat). Splitting the batch on every alternation cost one fsync per streamed
    /// token; grouping must keep each chat's order and sequence while paying one commit
    /// per chat instead.
    #[tokio::test]
    async fn interleaved_chats_batch_per_chat_and_keep_their_own_order() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        for n in 0..50 {
            log.enqueue(project, "member".into(), vec![json!({"n":n})])
                .unwrap();
            log.enqueue(project, "group".into(), vec![json!({"n":n})])
                .unwrap();
        }
        let terminal = log
            .append(project, "member".into(), vec![json!({"type":"done"})])
            .await
            .unwrap();
        assert_eq!(terminal[0]["seq"], 51);
        for (chat, expected) in [("member", 51), ("group", 50)] {
            let rows = log
                .read(project, chat.into(), 0, u64::MAX, 200)
                .await
                .unwrap();
            assert_eq!(rows.len(), expected, "{chat} lost rows");
            for (n, row) in rows.iter().enumerate().take(50) {
                assert_eq!(row["n"], n, "{chat} reordered its own rows");
                assert_eq!(row["seq"], n as u64 + 1, "{chat} sequence is not dense");
            }
        }
        let commits = log.0.commits.load(std::sync::atomic::Ordering::Relaxed);
        assert_eq!(
            commits, 2,
            "101 interleaved writes collapsed into {commits} commits, not one per chat"
        );
    }
    #[tokio::test]
    async fn background_write_failure_is_reported_and_fences_later_writes() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let log = ChatLog::open(file.path().into());
        let project = Uuid::new_v4();
        log.enqueue(project, "stream".into(), vec![json!({"content":"test"})])
            .unwrap();
        assert!(log.flush().await.is_err());
        assert!(
            log.enqueue(project, "stream".into(), vec![json!({})])
                .is_err()
        );
        assert!(
            log.append(project, "stream".into(), vec![json!({})])
                .await
                .is_err()
        );
        assert!(
            log.read(project, "stream".into(), 0, u64::MAX, 1)
                .await
                .is_err()
        );
    }
    #[tokio::test]
    async fn streaming_queue_is_bounded_without_silently_dropping_events() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        // This current-thread test does not yield while filling the channel.
        for n in 0..512 {
            log.enqueue(project, "stream".into(), vec![json!({"n":n})])
                .unwrap();
        }
        assert!(
            log.enqueue(project, "stream".into(), vec![json!({})])
                .is_err()
        );
        log.flush().await.unwrap();
        assert_eq!(
            log.read(project, "stream".into(), 0, u64::MAX, 1)
                .await
                .unwrap()[0]["seq"],
            512
        );
    }
    /// The live stream must not wait on the writer, and it must still see everything.
    /// A reader that took a write barrier on every poll was the reason a streaming
    /// response paid an fsync several times a second.
    #[tokio::test]
    async fn the_recent_read_serves_a_live_stream_without_a_write_barrier() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        for n in 0..50 {
            log.append(project, "stream".into(), vec![json!({"n":n})])
                .await
                .unwrap();
        }
        // Everything committed so far is answerable from memory, in order and complete.
        let all = log
            .read_recent(project, "stream".into(), 0, 100)
            .await
            .unwrap();
        assert_eq!(all.len(), 50);
        assert_eq!(all[0]["n"], 0);
        assert_eq!(all[49]["n"], 49);
        // A resuming reader asks for what is new, never re-sending what it has.
        let next = log
            .read_recent(project, "stream".into(), 50, 100)
            .await
            .unwrap();
        assert!(next.is_empty());
        let slice = log
            .read_recent(project, "stream".into(), 45, 100)
            .await
            .unwrap();
        assert_eq!(slice.len(), 5, "seq 46..=50 is five events");
        assert_eq!(slice[0]["seq"], 46);
        // A request reaching past the window falls back to the durable log.
        let old = log
            .read_recent(project, "other".into(), 0, 10)
            .await
            .unwrap();
        assert!(old.is_empty());
    }

    /// A request older than the memory window must still be answered, not dropped.
    #[tokio::test]
    async fn a_request_older_than_the_window_falls_back_to_the_log() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        let total = TAIL_CAPACITY + 500;
        // Enqueued in batches rather than committed one at a time: a durable commit per
        // event would spend the whole test in fsync, which is exactly the cost the
        // streaming path was changed to avoid. The queue is bounded, so it has to be
        // drained as it fills.
        for chunk in (0..total).collect::<Vec<_>>().chunks(256) {
            for n in chunk {
                log.enqueue(project, "long".into(), vec![json!({"n":n})])
                    .unwrap();
            }
            log.flush().await.unwrap();
        }
        let recent = log
            .read_recent(project, "long".into(), total as u64 - 10, 100)
            .await
            .unwrap();
        assert_eq!(recent.len(), 10, "the newest events must come from memory");
        assert_eq!(recent[0]["n"], total - 10);
        // Reaching into the evicted prefix reads through to disk.
        let old = log
            .read_recent(project, "long".into(), 5, 10)
            .await
            .unwrap();
        assert_eq!(old.len(), 10);
        assert_eq!(old[0]["seq"], 6);
    }

    /// A failed write must not leave memory claiming events the log never accepted.
    #[tokio::test]
    async fn a_failed_append_leaves_nothing_readable_behind() {
        let file = tempfile::NamedTempFile::new().unwrap();
        // A path that cannot hold a directory makes the very first append fail.
        std::fs::write(file.path(), b"blocked").unwrap();
        let log = ChatLog::open(file.path().join("nested").into());
        let project = Uuid::new_v4();
        assert!(
            log.append(project, "stream".into(), vec![json!({"n":1})])
                .await
                .is_err()
        );
        assert!(
            log.read_recent(project, "stream".into(), 0, 10)
                .await
                .is_err()
        );
    }

    /// The whole point of the memory window: the live poll must answer while the writer
    /// is busy, where the ordering read has to wait for it. The lock stands in for the
    /// commit that holds it, so this measures the barrier rather than disk timing.
    #[tokio::test]
    async fn the_streaming_poll_answers_while_the_ordering_read_waits() {
        let log = ChatLog::temporary();
        let project = Uuid::new_v4();
        for n in 0..20 {
            log.append(project, "stream".into(), vec![json!({"n":n})])
                .await
                .unwrap();
        }
        let disk = log.0.cursors.lock().unwrap();
        let started = std::time::Instant::now();
        let rows = log
            .read_recent(project, "stream".into(), 0, 100)
            .await
            .unwrap();
        let served = started.elapsed();
        drop(disk);
        // Serving from memory means the write lock was never needed at all.
        assert!(
            served < std::time::Duration::from_millis(50),
            "the streaming poll waited {served:?} on the writer"
        );
        assert_eq!(rows.len(), 20, "the poll lost events");
    }

    #[tokio::test]
    async fn append_pages_isolation_restart() {
        let dir = tempfile::tempdir().unwrap();
        let p = Uuid::new_v4();
        let log = ChatLog::open(dir.path().into());
        log.append(
            p,
            "../group".into(),
            vec![json!({"text":"一"}), json!({"text":"二"})],
        )
        .await
        .unwrap();
        let first = std::fs::read(
            files(&log.directory(p, "../group"))
                .unwrap()
                .first()
                .unwrap(),
        )
        .unwrap();
        let log = ChatLog::open(dir.path().into());
        assert_eq!(
            log.append(p, "../group".into(), vec![json!({"text":"三"})])
                .await
                .unwrap()[0]["seq"],
            3
        );
        assert!(
            std::fs::read(
                files(&log.directory(p, "../group"))
                    .unwrap()
                    .first()
                    .unwrap()
            )
            .unwrap()
            .starts_with(&first)
        );
        assert_eq!(
            log.read(p, "../group".into(), 0, 3, 1).await.unwrap()[0]["text"],
            "二"
        );
        assert!(
            log.read(Uuid::new_v4(), "../group".into(), 0, u64::MAX, 20)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(
            log.append(p, "../group".into(), vec![json!({"seq":1})])
                .await
                .is_err()
        );
    }
}
