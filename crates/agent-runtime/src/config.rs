//! Public configuration facade. Each provider owns its concrete configuration.
use crate::RuntimeKind;
pub use crate::providers::{
    carbot::config::{HarnessConfig, ModelApi},
    claude::config::ClaudeConfig,
    codex::config::CodexConfig,
};
use std::path::PathBuf;

pub(crate) fn workdir() -> PathBuf {
    crate::paths::workdir()
}

#[derive(Clone)]
pub enum RuntimeConfig {
    Mock,
    Carbot(HarnessConfig),
    Claude(ClaudeConfig),
    Codex(CodexConfig),
}
impl RuntimeConfig {
    /// Apply the host's execution-local directory uniformly for all providers.
    pub fn with_workspace(mut self) -> Self {
        if let Some(root) = crate::workspace::WorkspaceSettings::root() {
            match &mut self {
                Self::Mock => (),
                Self::Carbot(config) => config.root = root,
                Self::Codex(config) => config.workdir = root,
                Self::Claude(config) => config.workdir = root,
            }
        }
        self
    }
    pub fn kind(&self) -> RuntimeKind {
        match self {
            Self::Mock => RuntimeKind::Mock,
            Self::Carbot(_) => RuntimeKind::Carbot,
            Self::Codex(_) => RuntimeKind::Codex,
            Self::Claude(_) => RuntimeKind::Claude,
        }
    }
    pub fn from_env(kind: RuntimeKind) -> Result<Self, String> {
        Ok(match kind {
            RuntimeKind::Mock => Self::Mock,
            RuntimeKind::Carbot => Self::Carbot(HarnessConfig::from_env()?),
            RuntimeKind::Claude => Self::Claude(ClaudeConfig::from_env()),
            RuntimeKind::Codex => Self::Codex(CodexConfig::from_env()),
        })
    }
}
