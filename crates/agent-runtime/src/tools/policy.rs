use serde::{Deserialize, Serialize};
#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ToolPolicy {
    #[serde(default)]
    disabled: Vec<String>,
    #[serde(default)]
    external: Vec<ExternalCommand>,
}
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalCommand {
    name: String,
    description: String,
    command: String,
    enabled: bool,
}
impl ToolPolicy {
    pub fn disabled(&self) -> &[String] {
        &self.disabled
    }
    pub fn external(&self) -> &[ExternalCommand] {
        &self.external
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.disabled.len() > 256 || self.external.len() > 64 {
            return Err("tool policy limit exceeded".into());
        }
        let mut names = std::collections::HashSet::new();
        for item in &self.external {
            if item.name.is_empty()
                || item.name.len() > 64
                || !item
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || c == b'_')
                || !names.insert(&item.name)
            {
                return Err("invalid or duplicate external tool name".into());
            }
            if item.description.trim().is_empty() || item.description.len() > 2048 {
                return Err("tool description required (maximum 2 KiB)".into());
            }
            super::shell_execution::validate(&item.command)?;
        }
        Ok(())
    }
}
impl ExternalCommand {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn command(&self) -> &str {
        &self.command
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
}
