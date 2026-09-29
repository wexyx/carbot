use super::super::process::capture_stderr;
use super::config::ClaudeConfig;
use super::parser;
use crate::errors::{is_token_insufficient, token_limit_error};
use crate::{EventSink, RuntimeFuture, RuntimeKind};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

pub(crate) struct Runtime {
    config: ClaudeConfig,
    launch: super::super::launch_command::LaunchCommand,
}

impl Runtime {
    pub(crate) fn new(config: ClaudeConfig) -> Result<Self, String> {
        let launch = config.launch()?;
        Ok(Self { config, launch })
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
            let process = crate::execution::native::NativeCommand::new(&self.config.workdir)?;
            let credential = std::path::PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".claude/.credentials.json");
            if crate::environment::AgentEnvironment::lookup("ANTHROPIC_API_KEY").is_none()
                && crate::environment::AgentEnvironment::lookup("CLAUDE_CODE_OAUTH_TOKEN").is_none()
            {
                process
                    .import_credential(
                        &self.config.workdir,
                        &credential,
                        std::path::Path::new(".claude/.credentials.json"),
                    )
                    .await?;
            }
            let mut command = process.command(self.launch.binary(), &self.config.workdir)?;
            command.args(self.launch.arguments());
            for key in [
                "ANTHROPIC_API_KEY",
                "ANTHROPIC_BASE_URL",
                "CLAUDE_CODE_OAUTH_TOKEN",
            ] {
                if let Some(value) = crate::environment::AgentEnvironment::lookup(key) {
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
            command.args([
                "-p",
                "--output-format",
                "stream-json",
                "--verbose",
                "--include-partial-messages",
                "--permission-prompts",
                "none",
                "--permission-mode",
                mode,
            ]);
            let input = crate::attachments::PreparedAttachments::images(|images| {
                if images.is_empty() {
                    return None;
                }
                let mut content: Vec<Value> = images.iter().map(|image| serde_json::json!({"type":"image","source":{"type":"base64","media_type":image.media_type,"data":image.data}})).collect();
                content.push(serde_json::json!({"type":"text","text":prompt}));
                Some(format!(
                    "{}\n",
                    serde_json::json!({"type":"user","message":{"role":"user","content":content}})
                ))
            });
            if input.is_some() {
                command
                    .args(["--input-format", "stream-json"])
                    .stdin(Stdio::piped());
            } else {
                command.arg("--").arg(prompt);
            }
            let mut child = command
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
            let _group = crate::execution::native::ProcessGroup::new(
                child.id().ok_or("missing provider process ID")?,
            );
            let writer = input.map(|input| {
                let mut stdin = child.stdin.take().expect("piped Claude stdin");
                tokio::spawn(async move { stdin.write_all(input.as_bytes()).await })
            });
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
            if let Some(writer) = writer {
                writer
                    .await
                    .map_err(|e| e.to_string())?
                    .map_err(|e| format!("Claude image input failed: {e}"))?;
            }
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
