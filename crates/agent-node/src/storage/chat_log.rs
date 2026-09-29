//! Hourly JSONL files. Memory contains cursors, not conversation snapshots.
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use uuid::Uuid;
#[derive(Clone)]
pub(crate) struct ChatLog(Arc<Inner>);
struct Inner {
    root: PathBuf,
    cursors: Mutex<HashMap<PathBuf, u64>>,
    line_indexes: Mutex<HashMap<PathBuf, super::log_line_index::LineIndex>>,
    _temporary: Option<tempfile::TempDir>,
}
impl ChatLog {
    pub(crate) fn open(root: PathBuf) -> Self {
        Self(Arc::new(Inner {
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
        let log = self.clone();
        tokio::task::spawn_blocking(move || log.append_sync(project, &chat, events))
            .await
            .map_err(|e| e.to_string())?
    }
    fn append_sync(
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
        File::open(&dir)
            .and_then(|d| d.sync_all())
            .map_err(|e| e.to_string())?;
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
        let directory = self.directory(project, &chat);
        tokio::task::spawn_blocking(move || super::log_file_reader::read(&directory, &name, offset))
            .await
            .map_err(|e| e.to_string())?
    }
    pub(crate) async fn files(&self, project: Uuid, chat: String) -> Result<Vec<Value>, String> {
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
