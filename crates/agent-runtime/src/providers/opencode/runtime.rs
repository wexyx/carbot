use super::super::process::capture_stderr;
use super::config::OpenCodeConfig;
use super::parser;
use crate::errors::is_token_insufficient;
use crate::{EventSink, RuntimeFuture, RuntimeKind};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

pub struct Runtime {
    config: OpenCodeConfig,
    launch: super::super::launch_command::LaunchCommand,
}
/// How long OpenCode may stay silent before the run is treated as unreachable.
///
/// A client that cannot find its background service does not report that; it waits.
const FIRST_OUTPUT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);
const SILENT_ERROR: &str = "OpenCode produced no output; it cannot reach its background service. Enable the private server, or run `opencode service status`.";

impl Runtime {
    pub(crate) fn new(config: OpenCodeConfig) -> Result<Self, String> {
        let launch = config.launch()?;
        Ok(Self { config, launch })
    }
}

impl crate::providers::Provider for Runtime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::OpenCode
    }
    fn execute<'a>(
        &'a self,
        prompt: &'a str,
        on_event: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a> {
        Box::pin(async move {
            let process = crate::execution::native::NativeCommand::new(&self.config.workdir)?;
            let mut command = process.command(self.launch.binary(), &self.config.workdir)?;
            // Point OpenCode at the real config directory so its skills, plugins and
            // settings are the ones the operator installed. `~/.config/opencode` is small;
            // the multi-megabyte data directory is deliberately not shared, because that is
            // where sessions and caches live and the shared service registration is not
            // reachable through a config directory anyway.
            if let Some(config_dir) = super::super::home::config_dir(".config/opencode") {
                command.env("OPENCODE_CONFIG_DIR", config_dir);
            }
            // The shared service registers itself under XDG_STATE_HOME, which the scrubbed
            // environment leaves pointing at the scratch dir. A client that cannot find the
            // registration does not fail: it waits on starting a service that never
            // registers, and the run produces no output at all. Measured here: 45s with no
            // output against 2.2s once the real state directory is reachable.
            if let Some(state_dir) = super::super::home::config_dir(".local/state") {
                command.env("XDG_STATE_HOME", state_dir);
            }
            command.args(self.launch.arguments());
            for key in [
                "OPENCODE_API_KEY",
                "OPENCODE_CONFIG",
                "OPENCODE_CONFIG_DIR",
                "OPENCODE_CONFIG_CONTENT",
                "OPENCODE_PERMISSION",
            ] {
                if let Some(value) = crate::environment::AgentEnvironment::lookup(key) {
                    command.env(key, value);
                }
            }
            // Booting a private server keeps the run self-contained: the shared service
            // registers itself under the user's HOME, which this child does not have.
            let mut run = vec!["run".to_string()];
            if self.config.standalone {
                run.push("--standalone".into());
            }
            run.push("--format".into());
            run.push("json".into());
            command.args(&run);
            if self.config.thinking {
                // OpenCode omits deliberation parts without this flag, and reasoning
                // models answer before their reasoning shows.
                command.arg("--thinking");
            }
            if !self.config.model.is_empty() {
                command.args(["--model", &self.config.model]);
            }
            if !self.config.agent.is_empty() {
                command.args(["--agent", &self.config.agent]);
            }
            // Mirrors Claude's plan/bypass split: OpenCode's default asks for approval on
            // every tool call, which a non-interactive run can never answer.
            if self.config.auto_approve
                || crate::permissions::PermissionMode::current()
                    == crate::permissions::PermissionMode::Full
            {
                command.arg("--auto");
            }
            command.arg("--").arg(prompt);
            let mut child = command
                .current_dir(&self.config.workdir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|error| {
                    format!(
                        "Could not start OpenCode: {error}. Is opencode installed and logged in?"
                    )
                })?;
            let _group = crate::execution::native::ProcessGroup::new(
                child.id().ok_or("missing provider process ID")?,
            );
            let stdout = child.stdout.take().ok_or("OpenCode stdout unavailable")?;
            let stderr_task = tokio::spawn(capture_stderr(child.stderr.take()));
            let mut lines = BufReader::new(stdout).lines();
            let mut parser = parser::EventParser::default();
            // A client that cannot reach its service produces no output at all rather than
            // failing, so silence is bounded here instead of waiting forever.
            let mut first = true;
            loop {
                let line = if first {
                    first = false;
                    match tokio::time::timeout(FIRST_OUTPUT_TIMEOUT, lines.next_line()).await {
                        Err(_) => return Err(SILENT_ERROR.into()),
                        Ok(Err(error)) => return Err(error.to_string()),
                        Ok(Ok(None)) => break,
                        Ok(Ok(Some(line))) => line,
                    }
                } else {
                    match lines.next_line().await {
                        Err(error) => return Err(error.to_string()),
                        Ok(None) => break,
                        Ok(Some(line)) => line,
                    }
                };
                // `--format json` interleaves plain log lines with events, so every line
                // is a parse candidate and non-JSON is progress, not failure.
                parser.consume_line(&line, on_event);
            }
            let status = child.wait().await.map_err(|e| e.to_string())?;
            let stderr = stderr_task.await.unwrap_or_default();
            if is_token_insufficient(&stderr) {
                parser.error = Some(crate::errors::token_limit_error(&stderr));
            }
            if let Some(error) = parser.error {
                return Err(error);
            }
            if !status.success() {
                return Err(format!("OpenCode exited with {status}: {stderr}"));
            }
            Ok(parser.answer)
        })
    }
}
