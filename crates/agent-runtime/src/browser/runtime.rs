use crate::execution::native::{NativeCommand, ProcessGroup};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::io::AsyncReadExt;

/// The installer and Skill scripts share this exact resolver and readiness check.
pub(super) struct BrowserRuntime {
    data_dir: PathBuf,
}
impl BrowserRuntime {
    pub(super) fn new(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }
    pub(super) async fn ensure(&self, workdir: &Path) -> Result<PathBuf, String> {
        let ready = self
            .invoke(workdir, "console.log(JSON.stringify(await runtimeReady()))")
            .await?;
        if ready.trim() != "true" {
            crate::workspace::confirm_command(
                workdir,
                &format!(
                    "安装浏览器 Skill 依赖：Puppeteer 25.12.0 与 Chrome Headless Shell → {}",
                    self.data_dir
                        .join("runtime/browser-automation/.runtime")
                        .display()
                ),
                "browser-dependencies",
            )
            .await?;
            self.invoke(workdir, "await ensureRuntime()").await?;
        }
        Ok(self.data_dir.join("runtime/browser-automation/.runtime"))
    }
    async fn invoke(&self, workdir: &Path, operation: &str) -> Result<String, String> {
        let launcher = NativeCommand::new(workdir)?;
        let mut command = launcher.command(Path::new("node"), workdir)?;
        command
            .env("CARBOT_DATA_DIR", &self.data_dir)
            .args([
                "--input-type=module",
                "-e",
                &format!(
                    "{}\n{operation}",
                    include_str!(
                        "../../../../skills/system/business/browser-automation/runtime.mjs"
                    )
                ),
            ])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        let mut child = command
            .spawn()
            .map_err(|e| format!("Browser runtime requires Node.js and npm: {e}"))?;
        let _group = ProcessGroup::new(child.id().ok_or("missing browser installer PID")?);
        let stdout = child.stdout.take().ok_or("missing installer stdout")?;
        let stderr = child.stderr.take().ok_or("missing installer stderr")?;
        let (status, out, err) = tokio::try_join!(
            async { child.wait().await.map_err(|e| e.to_string()) },
            tail(stdout),
            tail(stderr)
        )?;
        if !status.success() {
            return Err(format!(
                "Browser runtime check/build failed ({status}): {err}"
            ));
        }
        Ok(out)
    }
}
async fn tail(mut reader: impl tokio::io::AsyncRead + Unpin) -> Result<String, String> {
    let mut bytes = Vec::new();
    let mut block = [0u8; 4096];
    loop {
        let count = reader.read(&mut block).await.map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        bytes.extend_from_slice(&block[..count]);
        if bytes.len() > 65536 {
            bytes.drain(..bytes.len() - 65536);
        }
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
