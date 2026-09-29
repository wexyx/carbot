use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::io::AsyncReadExt;

/// A fixed browser operation, not an unrestricted shell/script execution API.
pub(crate) struct BrowserExecution;
impl BrowserExecution {
    pub(crate) async fn run(root: &Path, url: &str, screenshot: bool) -> Result<Value, String> {
        let url = reqwest::Url::parse(url).map_err(|_| "invalid browser URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err("browser URL must be HTTP(S), without embedded credentials".into());
        }
        crate::workspace::confirm_browser(root, url.as_str()).await?;
        let runtime = super::runtime::BrowserRuntime::new(crate::paths::data_dir())
            .ensure(root)
            .await?;
        Self::execute(root, url.as_str(), screenshot, runtime).await
    }
    pub(super) async fn execute(
        root: &Path,
        url: &str,
        screenshot: bool,
        runtime: PathBuf,
    ) -> Result<Value, String> {
        let temporary = crate::workspace::temporary_dir(root)?;
        let profile = tempfile::Builder::new()
            .prefix("carbot-browser-")
            .tempdir_in(&temporary)
            .map_err(|e| e.to_string())?;
        let output = if screenshot {
            Some(
                tempfile::Builder::new()
                    .prefix("browser-output-")
                    .tempdir_in(&temporary)
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };
        let target = output.as_ref().map(|dir| dir.path().join("screenshot.png"));
        let mut command = tokio::process::Command::new("node");
        // Do not inherit NODE_OPTIONS, browser overrides, model secrets or personal profiles.
        command
            .current_dir(root)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", profile.path())
            .env("TMPDIR", profile.path())
            .env("MAC_CHROMIUM_TMPDIR", profile.path())
            .env("CFFIXED_USER_HOME", profile.path())
            .env(
                "LANG",
                std::env::var_os("LANG").unwrap_or_else(|| "en_US.UTF-8".into()),
            )
            .args(["--input-type=module", "-e", include_str!("bridge.mjs")])
            .arg(
                json!({"runtime":runtime,"profile":profile.path(),"url":url,"screenshot":target})
                    .to_string(),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        crate::environment::AgentEnvironment::apply(&mut command);
        let mut child = command
            .spawn()
            .map_err(|e| format!("browser requires Node.js: {e}"))?;
        let _group = crate::execution::native::ProcessGroup::new(
            child.id().ok_or("missing browser process ID")?,
        );
        let stdout = child.stdout.take().ok_or("missing browser stdout")?;
        let stderr = child.stderr.take().ok_or("missing browser stderr")?;
        let (status, stdout, stderr) = tokio::time::timeout(Duration::from_secs(120), async {
            tokio::try_join!(
                async { child.wait().await.map_err(|e| e.to_string()) },
                read(stdout),
                read(stderr)
            )
        })
        .await
        .map_err(|_| "browser operation timed out")??;
        if !status.success() {
            return Err(format!("browser failed ({status}): {stderr}"));
        }
        let mut result: Value =
            serde_json::from_str(&stdout).map_err(|e| format!("invalid browser result: {e}"))?;
        result["isolation"] = json!("browser_only_host_process_with_chromium_sandbox");
        if let Some(output) = output {
            let _ = output.keep();
        }
        Ok(result)
    }
}
async fn read(reader: impl tokio::io::AsyncRead + Unpin) -> Result<String, String> {
    let mut bytes = vec![];
    reader
        .take(65537)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("browser output exceeds 64 KiB".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
