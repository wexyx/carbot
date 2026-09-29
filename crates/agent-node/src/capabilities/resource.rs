use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct Resource {
    pub id: String,
    pub kind: String,
    pub scope: String,
    pub definition: Value,
    pub version: u64,
    pub readonly: bool,
    pub origin: Option<Value>,
}
impl Resource {
    pub fn name(&self) -> &str {
        self.definition[if self.kind == "skill" { "id" } else { "name" }]
            .as_str()
            .unwrap_or("")
    }
    pub fn default_enabled(&self, project: &str, agent: &str) -> bool {
        let enabled = self.definition["enabled"].as_bool().unwrap_or(true);
        match &self.origin {
            None => enabled,
            Some(origin) => {
                enabled
                    && origin["project"].as_str() == Some(project)
                    && origin["agent"].as_str().is_none_or(|id| id == agent)
            }
        }
    }
}
