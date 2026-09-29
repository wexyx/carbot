use super::SkillDefinition;
use serde::{Deserialize, Serialize};

/// A registered script plus arguments and explicit additional-path requests.
/// The trusted workspace is injected by the host, never deserialized from model input.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionRequest {
    #[serde(skip)]
    pub(crate) workdir: std::path::PathBuf,
    #[serde(default)]
    pub access: Vec<super::AccessRequest>,
    pub skill: SkillDefinition,
    pub path: String,
    pub args: Vec<String>,
    pub profile: String,
}
impl ExecutionRequest {
    pub fn validate(&self) -> Result<(), String> {
        if self.access.len() > 8
            || self
                .access
                .iter()
                .any(|a| a.path.is_empty() || a.path.len() > 4096)
        {
            return Err("invalid extra directory access request".into());
        }
        self.skill.validate()?;
        if !self.skill.enabled() || !self.skill.allow_python() {
            return Err("skill Python execution is not approved".into());
        }
        SkillDefinition::validate_path(&self.path)?;
        if !self.path.starts_with("scripts/")
            || !self.path.ends_with(".py")
            || !self.skill.files().contains_key(&self.path)
        {
            return Err("only registered scripts/*.py may execute".into());
        }
        if self.args.len() > 32 || self.args.iter().map(String::len).sum::<usize>() > 8192 {
            return Err("script arguments exceed limit".into());
        }
        Ok(())
    }
}
