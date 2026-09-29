use super::Resource;
use crate::storage::Store;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Context {
    pub project: String,
    pub namespace: String,
    pub agent: String,
}
impl Context {
    pub fn new(project: uuid::Uuid, agent: &str) -> Self {
        Self {
            project: project.to_string(),
            namespace: project.to_string(),
            agent: agent.into(),
        }
    }
    pub fn in_group(mut self, group: Option<&str>) -> Self {
        if let Some(group) = group.filter(|g| !g.is_empty()) {
            self.project = group.into();
        }
        self
    }
    pub fn key(&self, id: &str, layer: &str) -> Result<String, String> {
        let (project, agent) = match layer {
            "global" => ("", ""),
            "project" => (self.project.as_str(), ""),
            "agent" => ("", self.agent.as_str()),
            "project_agent" => (self.project.as_str(), self.agent.as_str()),
            _ => return Err("unknown capability layer".into()),
        };
        if matches!(layer, "agent" | "project_agent") && agent.is_empty() {
            return Err("Agent required".into());
        }
        Ok(json!([id, layer, project, agent]).to_string())
    }
    pub async fn resolve(
        &self,
        store: &Store,
        resource: &Resource,
        fallback: Option<bool>,
    ) -> Result<Value, String> {
        let mut enabled =
            fallback.unwrap_or_else(|| resource.default_enabled(&self.namespace, &self.agent));
        let mut source = "default";
        let mut layers = vec![];
        for layer in ["global", "project", "agent", "project_agent"] {
            if self.agent.is_empty() && matches!(layer, "agent" | "project_agent") {
                continue;
            }
            let row = store
                .get("capability_bindings", &self.key(&resource.id, layer)?)
                .await
                .unwrap_or(json!({"version":0,"enabled":null}));
            let inherited =
                if self.project != self.namespace && matches!(layer, "project" | "project_agent") {
                    let mut legacy = self.clone();
                    legacy.project = legacy.namespace.clone();
                    store
                        .get("capability_bindings", &legacy.key(&resource.id, layer)?)
                        .await
                        .and_then(|r| r["enabled"].as_bool())
                } else {
                    None
                };
            if let Some(value) = row["enabled"].as_bool().or(inherited) {
                enabled = value;
                source = layer
            }
            layers.push(json!({"layer":layer,"enabled":row["enabled"],"version":row["version"]}));
        }
        Ok(json!({"enabled":enabled,"source":source,"layers":layers}))
    }
}
