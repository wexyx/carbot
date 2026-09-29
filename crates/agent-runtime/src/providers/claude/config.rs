use crate::config::workdir;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct ClaudeConfig {
    pub binary: PathBuf,
    pub permission_mode: String,
    pub workdir: PathBuf,
}
impl ClaudeConfig {
    pub fn from_env() -> Self {
        Self {
            binary: env::var("CLAUDE_BIN")
                .unwrap_or_else(|_| "claude".into())
                .into(),
            permission_mode: env::var("CLAUDE_PERMISSION_MODE").unwrap_or_else(|_| "plan".into()),
            workdir: workdir(),
        }
    }
}
