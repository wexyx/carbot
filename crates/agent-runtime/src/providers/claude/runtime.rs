use super::super::process::capture_stderr;
use super::config::ClaudeConfig;
use super::parser;
use crate::errors::{is_token_insufficient, token_limit_error};
use crate::{EventSink, RuntimeFuture, RuntimeKind};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

pub(crate) struct Runtime {
    config: ClaudeConfig,
}

impl Runtime {
    pub(crate) fn new(config: ClaudeConfig) -> Result<Self, String> {
        Ok(Self { config })
    }
}

impl crate::providers::Provider for Runtime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Claude
    }
    fn execute<'a>(
        &'a self,
        prompt: &'a str,
        on_event: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a> {
        Box::pin(async move {
            let sandbox = crate::sandbox::native::NativeCommand::new()?;
            let credential = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".claude/.credentials.json");
            if std::env::var_os("ANTHROPIC_API_KEY").is_none() {
                sandbox
                    .import_credential(
                        &self.config.workdir,
                        &credential,
                        std::path::Path::new(".claude/.credentials.json"),
                    )
                    .await?;
            }
            let mut command =
                sandbox.command(&self.config.binary, &self.config.workdir, &[], &[], true)?;
            for key in [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_BASE_URL",
                "CLAUDE_CODE_OAUTH_TOKEN",
            ] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
            let mode = if crate::permissions::PermissionMode::current()
                == crate::permissions::PermissionMode::Full
            {
                "bypassPermissions"
            } else {
                "plan"
            };
            let mut child = command
                .args([
                    "-p",
                    "--output-format",
                    "stream-json",
                    "--verbose",
                    "--include-partial-messages",
                    "--permission-prompts",
                    "none",
                    "--permission-mode",
                    mode,
                    prompt,
                ])
                .current_dir(&self.config.workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|error| {
                    format!(
                        "Could not start Claude Code: {error}. Is claude installed and logged in?"
                    )
                })?;
            let _group = crate::sandbox::native::ProcessGroup::new(
                child.id().ok_or("missing provider process ID")?,
            );
            let stdout = child.stdout.take().ok_or("Claude stdout unavailable")?;
            let stderr_task = tokio::spawn(capture_stderr(child.stderr.take()));
            let mut lines = BufReader::new(stdout).lines();
            let mut parser = parser::EventParser::default();
            while let Some(line) = lines.next_line().await.map_err(|e| e.to_string())? {
                if let Ok(value) = serde_json::from_str::<Value>(&line) {
                    parser.consume(&value, on_event);
                }
            }
            let status = child.wait().await.map_err(|e| e.to_string())?;
            let stderr = stderr_task.await.unwrap_or_default();
            if is_token_insufficient(&stderr) {
                parser.error = Some(token_limit_error(&stderr));
            }
            if let Some(error) = parser.error {
                return Err(error);
            }
            if !status.success() {
                return Err(format!("Claude Code exited with {status}: {stderr}"));
            }
            Ok(parser.answer)
        })
    }
}
