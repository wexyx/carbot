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
    pub(crate) fn environment(&self) -> crate::environment::AgentEnvironment {
        match self {
            Self::Mock => Default::default(),
            Self::Carbot(cfg) => cfg.environment.clone(),
            Self::Codex(cfg) => cfg.environment.clone(),
            Self::Claude(cfg) => cfg.environment.clone(),
        }
    }
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
        let environment = crate::environment::AgentEnvironment::from_json(
            &std::env::var("AGENT_ENV_JSON").unwrap_or_default(),
        )?;
        Ok(match kind {
            RuntimeKind::Mock => Self::Mock,
            RuntimeKind::Carbot => Self::Carbot(HarnessConfig::from_env()?),
            RuntimeKind::Claude => {
                let mut cfg = ClaudeConfig::from_env();
                cfg.environment = environment;
                Self::Claude(cfg)
            }
            RuntimeKind::Codex => {
                let mut cfg = CodexConfig::from_env();
                cfg.environment = environment;
                Self::Codex(cfg)
            }
        })
    }
}
