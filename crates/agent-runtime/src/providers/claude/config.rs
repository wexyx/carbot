use crate::config::workdir;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct ClaudeConfig {
    pub binary: PathBuf,
    pub environment: crate::environment::AgentEnvironment,
    pub permission_mode: String,
    pub workdir: PathBuf,
}
impl ClaudeConfig {
    pub fn validate_launch(&self) -> Result<(), String> {
        self.launch().map(|_| ())
    }
    pub(crate) fn launch(&self) -> Result<super::super::launch_command::LaunchCommand, String> {
        let command = super::super::launch_command::LaunchCommand::parse(&self.binary)?;
        command.reject(&[
            "-p",
            "--print",
            "--output-format",
            "--input-format",
            "--permission-mode",
            "--permission-prompts",
            "--dangerously-skip-permissions",
            "--allow-dangerously-skip-permissions",
            "--resume",
            "-r",
            "--continue",
            "-c",
            "--session-id",
        ])?;
        Ok(command)
    }
    pub fn from_env() -> Self {
        Self {
            environment: Default::default(),
            binary: env::var("CLAUDE_BIN")
                .unwrap_or_else(|_| "claude".into())
                .into(),
            permission_mode: env::var("CLAUDE_PERMISSION_MODE").unwrap_or_else(|_| "plan".into()),
            workdir: workdir(),
        }
    }
}
