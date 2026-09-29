use crate::config::workdir;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct CodexConfig {
    pub binary: PathBuf,
    pub environment: crate::environment::AgentEnvironment,
    pub sandbox: String,
    pub workdir: PathBuf,
}
impl CodexConfig {
    pub fn validate_launch(&self) -> Result<(), String> {
        self.launch().map(|_| ())
    }
    pub(crate) fn launch(&self) -> Result<super::super::launch_command::LaunchCommand, String> {
        let command = super::super::launch_command::LaunchCommand::parse(&self.binary)?;
        command.reject(&[
            "exec",
            "e",
            "resume",
            "--json",
            "--ephemeral",
            "--sandbox",
            "-s",
            "--ask-for-approval",
            "-a",
            "--full-auto",
            "--dangerously-bypass-approvals-and-sandbox",
            "--yolo",
            "--cd",
            "-C",
        ])?;
        Ok(command)
    }
    pub fn from_env() -> Self {
        Self {
            environment: Default::default(),
            binary: env::var("CODEX_BIN")
                .unwrap_or_else(|_| "codex".into())
                .into(),
            sandbox: env::var("CODEX_SANDBOX").unwrap_or_else(|_| "read-only".into()),
            workdir: workdir(),
        }
    }
}
