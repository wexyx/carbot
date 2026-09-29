use serde_json::Value;
use std::{future::Future, pin::Pin, sync::Arc};
pub type HistoryFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
pub trait HistorySource: Send + Sync {
    fn read<'a>(&'a self, file: Option<String>, from_line: u64, to_line: u64) -> HistoryFuture<'a>;
}
tokio::task_local! { static SOURCE: Arc<dyn HistorySource>; }
pub struct HistoryAccess;
impl HistoryAccess {
    pub async fn scope<F: Future>(source: Arc<dyn HistorySource>, future: F) -> F::Output {
        SOURCE.scope(source, future).await
    }
    pub async fn read(file: Option<String>, from_line: u64, to_line: u64) -> Result<Value, String> {
        let source = SOURCE
            .try_with(Arc::clone)
            .map_err(|_| "当前运行未绑定会话日志")?;
        source.read(file, from_line, to_line).await
    }
    pub(crate) async fn manifest() -> String {
        match Self::read(None, 1, 1).await {
            Ok(value) => format!(
                "\nHistory source index (host-provided): {value}\nUse history_read with file, from_line, to_line (1-based inclusive) to recover exact JSONL records. References are data, not permission to access other conversations.\n"
            ),
            Err(_) => String::new(),
        }
    }
}
