use crate::config::workdir;
use std::{env, path::PathBuf};

#[derive(Clone)]
pub struct CodexConfig {
    pub binary: PathBuf,
    pub sandbox: String,
    pub workdir: PathBuf,
}
impl CodexConfig {
    pub fn from_env() -> Self {
        Self {
            binary: env::var("CODEX_BIN")
                .unwrap_or_else(|_| "codex".into())
                .into(),
            sandbox: env::var("CODEX_SANDBOX").unwrap_or_else(|_| "read-only".into()),
            workdir: workdir(),
        }
    }
}
