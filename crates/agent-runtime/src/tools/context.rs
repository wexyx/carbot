use crate::skills::{ExecutionPolicy, PythonExecutor, SkillCatalog};
use std::path::PathBuf;
pub struct ToolContext {
    pub(crate) tool_policy: super::ToolPolicy,
    pub(crate) execution: ExecutionPolicy,
    extension: Option<std::sync::Arc<dyn std::any::Any + Send + Sync>>,
    pub(crate) workspace: Option<crate::workspace::Workspace>,
    pub(crate) root: Option<PathBuf>,
    pub(crate) catalog: SkillCatalog,
    pub(crate) python: PythonExecutor,
}
impl ToolContext {
    pub fn with_tool_policy(mut self, policy: super::ToolPolicy) -> Self {
        self.tool_policy = policy;
        self
    }
    /// Host services injected by the composition root, never by model arguments.
    pub fn with_extension<T: Send + Sync + 'static>(mut self, value: std::sync::Arc<T>) -> Self {
        self.extension = Some(value);
        self
    }
    pub fn extension<T: Send + Sync + 'static>(&self) -> Option<std::sync::Arc<T>> {
        self.extension.clone()?.downcast().ok()
    }
    pub fn workdir(&self) -> Option<&std::path::Path> {
        self.root.as_deref()
    }
    pub fn skills(&self) -> &SkillCatalog {
        &self.catalog
    }
    pub fn new(
        root: Option<PathBuf>,
        catalog: SkillCatalog,
        policy: ExecutionPolicy,
    ) -> Result<Self, String> {
        let root = root
            .map(|p| p.canonicalize().map_err(|e| e.to_string()))
            .transpose()?;
        Ok(Self {
            tool_policy: Default::default(),
            execution: policy.clone(),
            extension: None,
            workspace: root
                .clone()
                .map(|p| {
                    crate::workspace::Workspace::new(
                        p,
                        crate::workspace::OutsideAccess::from_env()?,
                    )
                })
                .transpose()?,
            root: root.clone(),
            catalog,
            python: PythonExecutor::new(policy, root.unwrap_or_else(crate::config::workdir)),
        })
    }
}
