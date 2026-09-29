use super::{ToolContext, ToolSession};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    env,
    path::{Path, PathBuf},
    sync::Arc,
};
fn protect_path(resolved: &Path) -> Result<(), String> {
    if std::fs::canonicalize(env::var("CARBOT_DATA_DIR").unwrap_or_else(|_| ".carbot".into()))
        .ok()
        .is_some_and(|data| resolved.starts_with(data))
    {
        return Err("Carbot private state cannot be read by model tools".into());
    }
    let credentials =
        env::var("CLIENT_CREDENTIALS_FILE").unwrap_or_else(|_| ".gateway-client.json".into());
    if std::fs::canonicalize(credentials).ok().as_ref() == Some(&resolved.to_path_buf()) {
        return Err("client credentials cannot be read by model tools".into());
    }
    for part in resolved.components() {
        let name = part.as_os_str().to_string_lossy();
        if name.starts_with(".env")
            || [
                ".git",
                ".ssh",
                ".aws",
                ".client.env",
                ".node.env",
                ".agent.env",
                ".sandbox.env",
                ".carbot",
                ".gateway-client.json",
            ]
            .contains(&name.as_ref())
            || name.ends_with(".pem")
            || name.ends_with(".key")
        {
            return Err("credential/configuration paths are not exposed as tools".into());
        }
    }
    Ok(())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathArgs {
    path: String,
}
async fn resolve(context: &ToolContext, args: &Value, operation: &str) -> Result<PathBuf, String> {
    let args: PathArgs = serde_json::from_value(args.clone()).map_err(|e| e.to_string())?;
    let workspace = context
        .workspace
        .as_ref()
        .ok_or("filesystem tools disabled")?;
    let path = workspace
        .root()
        .join(&args.path)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    protect_path(&path)?;
    workspace.authorize(Path::new(&args.path), operation).await
}

struct ReadFile {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name = "read_file", description = "Read a UTF-8 file up to 64 KiB.", parameters = json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}), runtime = crate)]
impl ReadFile {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, args: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let path = resolve(&self.context, args, "read_file").await?;
        let result: Result<String, String> = async {
            use tokio::io::AsyncReadExt;
            let file = tokio::fs::File::open(path)
                .await
                .map_err(|e| e.to_string())?;
            let metadata = file.metadata().await.map_err(|e| e.to_string())?;
            if !metadata.is_file() {
                return Err("only regular files can be read".into());
            }
            let mut bytes = Vec::new();
            file.take(65537)
                .read_to_end(&mut bytes)
                .await
                .map_err(|e| e.to_string())?;
            if bytes.len() > 65536 {
                return Err("file exceeds 64 KiB tool limit".into());
            }
            String::from_utf8(bytes).map_err(|_| "file is not UTF-8 text".into())
        }
        .await;
        result.map(Value::String)
    }
}

struct ListFiles {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name = "list_files", description = "List up to 200 working-directory entries.", parameters = json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}), runtime = crate)]
impl ListFiles {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, args: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let path = resolve(&self.context, args, "list_files").await?;
        let result: Result<String, String> = async {
            let mut entries = tokio::fs::read_dir(path).await.map_err(|e| e.to_string())?;
            let mut names = Vec::new();
            while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
                names.push(entry.file_name().to_string_lossy().to_string());
                if names.len() == 200 {
                    break;
                }
            }
            names.sort();
            Ok(json!({"entries":names,"limit":200}).to_string())
        }
        .await;
        result.map(Value::String)
    }
}
