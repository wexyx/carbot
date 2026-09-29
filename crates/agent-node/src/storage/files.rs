//! Plain JSON snapshots. No SQL engine or external service.
use crate::*;
use fs2::FileExt;
use std::{
    fs::{File, OpenOptions},
    io::Write,
    path::{Path as FilePath, PathBuf},
};

#[derive(Clone, Serialize, Deserialize)]
pub struct Data {
    format_version: u32,
    pub sequence: u64,
    pub collections: HashMap<String, HashMap<String, Value>>,
}
impl Default for Data {
    fn default() -> Self {
        Self {
            format_version: 1,
            sequence: 0,
            collections: HashMap::new(),
        }
    }
}
impl Data {
    pub fn get(&self, collection: &str, key: &str) -> Option<&Value> {
        self.collections.get(collection)?.get(key)
    }
    pub fn list(&self, collection: &str) -> Vec<Value> {
        self.collections
            .get(collection)
            .map(|c| c.values().cloned().collect())
            .unwrap_or_default()
    }
    pub fn set(&mut self, collection: &str, key: &str, value: Value) {
        self.collections
            .entry(collection.into())
            .or_default()
            .insert(key.into(), value);
    }
    pub fn insert(&mut self, collection: &str, key: &str, value: Value) -> Result<(), String> {
        if self.get(collection, key).is_some() {
            return Err("already_exists".into());
        }
        self.set(collection, key, value);
        Ok(())
    }
    pub fn credential(&mut self, value: Value) -> Result<(), String> {
        if self
            .list("credentials")
            .iter()
            .any(|c| c["project_id"] == value["project_id"] && c["client_id"] == value["client_id"])
        {
            return Err("already_exists".into());
        }
        let key = field(&value, "ak");
        self.insert("credentials", &key, value)
    }
}
struct Inner {
    logs: super::ChatLog,
    data: std::sync::Mutex<Data>,
    file: Option<PathBuf>,
    // OS advisory lock is released even after a crash; never unlink the lock file.
    _lock: Option<File>,
}
#[derive(Clone)]
pub struct Store(Arc<Inner>);
impl Store {
    #[cfg(test)]
    pub fn memory() -> Self {
        Self(Arc::new(Inner {
            logs: super::ChatLog::temporary(),
            data: Default::default(),
            file: None,
            _lock: None,
        }))
    }
    pub(crate) fn logs(&self) -> &super::ChatLog {
        &self.0.logs
    }
    pub async fn get(&self, c: &str, k: &str) -> Option<Value> {
        self.0.data.lock().unwrap().get(c, k).cloned()
    }
    pub async fn list(&self, c: &str) -> Vec<Value> {
        self.0.data.lock().unwrap().list(c)
    }
    pub async fn insert(&self, c: &str, k: &str, v: Value) -> Result<(), String> {
        self.transaction(|d| d.insert(c, k, v)).await
    }
    /// No await between durable commit and publishing memory: cancellation cannot split them.
    pub async fn transaction<T>(
        &self,
        change: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<T, String> {
        self.transaction_sync(change)
    }
    fn transaction_sync<T>(
        &self,
        change: impl FnOnce(&mut Data) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut data = self.0.data.lock().map_err(|_| "file store lock poisoned")?;
        let mut next = data.clone();
        let result = change(&mut next)?;
        if let Some(path) = &self.0.file {
            atomic_save(path, &next).map_err(|e| format!("save local file: {e}"))?;
        }
        *data = next;
        Ok(result)
    }
}
fn private_file(path: &FilePath, exclusive: bool) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    if exclusive {
        options.create_new(true);
    } else {
        options.create(true).truncate(false);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}
fn atomic_save(path: &FilePath, data: &Data) -> std::io::Result<()> {
    let temporary = path.with_file_name(format!("state.{}.tmp", Uuid::new_v4()));
    let result = (|| {
        let mut file = private_file(&temporary, true)?;
        serde_json::to_writer_pretty(&mut file, data)?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)?;
        // The rename is already committed. Keep memory consistent if directory fsync fails.
        #[cfg(unix)]
        if let Some(parent) = path.parent() {
            if let Err(e) = File::open(parent).and_then(|d| d.sync_all()) {
                eprintln!("warning: directory sync failed: {e}");
            }
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}
pub async fn open(dir: &FilePath) -> Result<Store, String> {
    if !dir.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(dir).map_err(|e| e.to_string())?;
    }
    let mut lock = private_file(&dir.join("store.lock"), false).map_err(|e| e.to_string())?;
    lock.try_lock_exclusive().map_err(|_| {
        let owner=std::fs::read_to_string(dir.join("store.lock")).unwrap_or_default();
        format!("data directory {} is already in use by another Carbot ({owner}). Use another --name or close the existing instance.",dir.display())
    })?;
    lock.set_len(0).map_err(|e| e.to_string())?;
    write!(
        lock,
        "pid={} instance={}",
        std::process::id(),
        std::env::var("CARBOT_INSTANCE").unwrap_or_else(|_| "default".into())
    )
    .map_err(|e| e.to_string())?;
    lock.sync_all().map_err(|e| e.to_string())?;
    let path = dir.join("state.json");
    let mut data = match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice::<Data>(&bytes)
            .map_err(|e| format!("invalid state.json; refusing to overwrite: {e}"))?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Data::default(),
        Err(e) => return Err(e.to_string()),
    };
    if data.format_version != 1 {
        return Err("unsupported state.json format version".into());
    }
    if !path.exists() {
        atomic_save(&path, &data).map_err(|e| e.to_string())?;
    }
    let logs = super::ChatLog::open(dir.join("chats"));
    if super::chat_migration::migrate(dir, &mut data, &logs).await? {
        atomic_save(&path, &data).map_err(|e| e.to_string())?;
    }
    Ok(Store(Arc::new(Inner {
        logs,
        data: std::sync::Mutex::new(data),
        file: Some(path),
        _lock: Some(lock),
    })))
}
pub fn field(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or_default().into()
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub async fn restore(state: &AppState) {
    for row in state.store.list("sessions").await {
        let id = Uuid::parse_str(&field(&row, "id")).expect("session ID");
        let (events, _) = broadcast::channel(256);
        state.sessions.lock().await.insert(
            id,
            Session {
                project_id: Uuid::parse_str(&field(&row, "project_id")).expect("project ID"),
                client_id: row["client_id"].as_str().map(str::to_owned),
                events,
                requests: HashMap::new(),
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            Self(std::env::temp_dir().join(format!("carbot-files-test-{}", Uuid::new_v4())))
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[tokio::test]
    async fn json_reopens_and_exclusive_lock_releases() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        assert!(open(&dir.0).await.is_err());
        store
            .insert("sessions", "a", json!({"content":"你好"}))
            .await
            .unwrap();
        let on_disk: Value =
            serde_json::from_slice(&std::fs::read(dir.0.join("state.json")).unwrap()).unwrap();
        assert_eq!(on_disk["collections"]["sessions"]["a"]["content"], "你好");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(dir.0.join("state.json"))
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        drop(store);
        let again = open(&dir.0).await.unwrap();
        assert_eq!(again.get("sessions", "a").await.unwrap()["content"], "你好");
    }
    #[tokio::test]
    async fn corrupt_file_is_not_silently_reset() {
        let dir = Temp::new();
        drop(open(&dir.0).await.unwrap());
        std::fs::write(dir.0.join("state.json"), b"invalid-json").unwrap();
        assert!(open(&dir.0).await.is_err());
        assert_eq!(
            std::fs::read(dir.0.join("state.json")).unwrap(),
            b"invalid-json"
        );
    }
    #[tokio::test]
    async fn failed_transaction_and_failed_disk_write_do_not_change_memory() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        store.insert("test", "a", json!(1)).await.unwrap();
        let failed: Result<(), String> = store
            .transaction(|d| {
                d.set("test", "a", json!(2));
                Err("reject".into())
            })
            .await;
        assert!(failed.is_err());
        assert_eq!(store.get("test", "a").await, Some(json!(1)));
        std::fs::rename(dir.0.join("state.json"), dir.0.join("saved.json")).unwrap();
        std::fs::create_dir(dir.0.join("state.json")).unwrap();
        assert!(store.insert("test", "b", json!(3)).await.is_err());
        assert!(store.get("test", "b").await.is_none());
        assert_eq!(store.get("test", "a").await, Some(json!(1)));
    }
    #[tokio::test]
    async fn file_backed_policy_cas_is_atomic_and_project_scoped() {
        let dir = Temp::new();
        let store = open(&dir.0).await.unwrap();
        let policy = policy_store::Store::files(store.clone());
        let p = Uuid::new_v4();
        policy
            .put(p, "subgroup", "g", 0, json!({"mode":"relay"}))
            .await
            .unwrap();
        let (a, b) = tokio::join!(
            policy.put(p, "subgroup", "g", 1, json!({"v":"a"})),
            policy.put(p, "subgroup", "g", 1, json!({"v":"b"}))
        );
        assert_ne!(a.is_ok(), b.is_ok());
        assert!(policy.get(Uuid::new_v4(), "subgroup", "g").await.is_err());
        drop(policy);
        drop(store);
        let reloaded = policy_store::Store::files(open(&dir.0).await.unwrap());
        assert_eq!(reloaded.get(p, "subgroup", "g").await.unwrap().version, 2);
    }
}
