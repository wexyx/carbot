use crate::{AgentRuntime, config::RuntimeConfig, providers};

/// The only place that constructs concrete execution providers.
pub struct RuntimeFactory;
impl RuntimeFactory {
    pub fn from_env() -> Result<Box<dyn AgentRuntime>, String> {
        Self::create(&std::env::var("AGENT_PROVIDER").unwrap_or_else(|_| "mock".into()))
    }
    pub fn create(provider: &str) -> Result<Box<dyn AgentRuntime>, String> {
        Self::from_config(RuntimeConfig::from_env(provider.parse()?)?)
    }
    /// Explicit configuration avoids mutating process environment in tests or embedders.
    pub fn from_config(config: RuntimeConfig) -> Result<Box<dyn AgentRuntime>, String> {
        Self::from_config_with_skills(
            config,
            crate::skills::SkillCatalog::default(),
            crate::skills::ExecutionPolicy::new("offline".into(), false)?,
        )
    }
    pub fn create_with_skills(
        provider: &str,
        catalog: crate::skills::SkillCatalog,
        policy: crate::skills::ExecutionPolicy,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        Self::from_config_with_skills(RuntimeConfig::from_env(provider.parse()?)?, catalog, policy)
    }
    pub fn from_config_with_skills(
        config: RuntimeConfig,
        catalog: crate::skills::SkillCatalog,
        policy: crate::skills::ExecutionPolicy,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        Self::from_config_with_policy(config, catalog, policy, crate::tools::ToolPolicy::default())
    }
    pub fn from_config_with_policy(
        config: RuntimeConfig,
        catalog: crate::skills::SkillCatalog,
        policy: crate::skills::ExecutionPolicy,
        tools: crate::tools::ToolPolicy,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        let root = match &config {
            RuntimeConfig::Carbot(cfg) => Some(cfg.root.clone()),
            RuntimeConfig::Codex(cfg) => Some(cfg.workdir.clone()),
            RuntimeConfig::Claude(cfg) => Some(cfg.workdir.clone()),
            _ => {
                if tools.external().is_empty() {
                    None
                } else {
                    Some(crate::config::workdir())
                }
            }
        };
        let manifest = catalog.manifest();
        let registry = crate::tools::ToolFactory::create(
            crate::tools::ToolContext::new(root, catalog, policy)?.with_tool_policy(tools),
        )?;
        Self::build(config, registry, manifest, None)
    }
    /// Inject a registration snapshot. Custom tools are trusted host code, not model-controlled registrations.
    pub fn from_config_with_tools(
        config: RuntimeConfig,
        registry: crate::tools::ToolRegistry,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        Self::build(config, registry, serde_json::json!([]), None)
    }
    /// Management CLI providers get an empty, ephemeral workspace, never the business workdir.
    /// Host mutations remain mediated by the injected management registry and human approvals.
    pub fn for_management(
        mut config: RuntimeConfig,
        registry: crate::tools::ToolRegistry,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        let workspace = tempfile::Builder::new()
            .prefix("carbot-admin-")
            .tempdir()
            .map_err(|e| e.to_string())?;
        match &mut config {
            RuntimeConfig::Codex(cfg) => {
                cfg.workdir = workspace.path().into();
                cfg.sandbox = "read-only".into();
            }
            RuntimeConfig::Claude(cfg) => {
                cfg.workdir = workspace.path().into();
                cfg.permission_mode = "plan".into();
            }
            RuntimeConfig::Carbot(cfg) => {
                cfg.root = workspace.path().into();
            }
            RuntimeConfig::Mock => {}
        }
        Self::build(config, registry, serde_json::json!([]), Some(workspace))
    }
    fn build(
        config: RuntimeConfig,
        registry: crate::tools::ToolRegistry,
        manifest: serde_json::Value,
        workspace: Option<tempfile::TempDir>,
    ) -> Result<Box<dyn AgentRuntime>, String> {
        let attachment_root = match &config {
            RuntimeConfig::Carbot(cfg) => cfg.root.clone(),
            RuntimeConfig::Codex(cfg) => cfg.workdir.clone(),
            RuntimeConfig::Claude(cfg) => cfg.workdir.clone(),
            RuntimeConfig::Mock => {
                crate::workspace::WorkspaceSettings::root().unwrap_or_else(crate::config::workdir)
            }
        };
        let provider: Box<dyn providers::Provider> = match config {
            RuntimeConfig::Mock => Box::new(providers::mock::Runtime::new()),
            RuntimeConfig::Carbot(config) => Box::new(providers::carbot::Runtime::new(
                config,
                registry.clone(),
                manifest.clone(),
            )?),
            RuntimeConfig::Claude(config) => Box::new(providers::claude::Runtime::new(config)?),
            RuntimeConfig::Codex(config) => Box::new(providers::codex::Runtime::new(config)?),
        };
        let provider = if provider.handles_tools() || registry.definitions().is_empty() {
            provider
        } else {
            Box::new(crate::tools::ToolRuntime::new(provider, registry, manifest))
                as Box<dyn providers::Provider>
        };
        Ok(Box::new(
            crate::managed_runtime::ManagedRuntime::new(provider)
                .with_attachment_root(attachment_root)
                .retain_workspace(workspace),
        ))
    }
}
