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
pub(super) struct Inner {
    writer: OnceLock<super::log_writer::LogWriter>,
    root: PathBuf,
    cursors: Mutex<HashMap<PathBuf, u64>>,
    line_indexes: Mutex<HashMap<PathBuf, super::log_line_index::LineIndex>>,
    _temporary: Option<tempfile::TempDir>,
}
impl ChatLog {
    pub(crate) fn open(root: PathBuf) -> Self {
        Self(Arc::new(Inner {
            writer: OnceLock::new(),
            root,
            cursors: Mutex::new(HashMap::new()),
            line_indexes: Mutex::new(HashMap::new()),
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
            _temporary: Some(dir),
        }))
    }
    fn directory(&self, project: Uuid, chat: &str) -> PathBuf {
        let key = chat
            .as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
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
        builder.create(&dir).map_err(|e| e.to_string())?;
        let path = dir.join(format!("hour-{:012}.jsonl", now / 3600));
        let is_new_file = !path.exists();
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
            return Err(format!("append chat log: {error}"));
        }
        cursors.insert(dir.clone(), seq);
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
