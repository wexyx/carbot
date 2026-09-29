use super::{Tool, ToolDefinition, ToolSession};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
/// Clone creates a registration snapshot, not a shared mutable global registry.
#[derive(Clone, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, (ToolDefinition, Arc<dyn Tool>)>,
}
impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn register(&mut self, tool: Arc<dyn Tool>) -> Result<(), String> {
        let definition = tool.definition();
        let name = definition.name();
        if name.is_empty()
            || name.len() > 64
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(
                "tool names must use ASCII letters, numbers or underscores, at most 64 characters"
                    .into(),
            );
        }
        if self.tools.contains_key(name) {
            return Err(format!("duplicate tool: {name}"));
        }
        if definition.parameters()["type"] != "object" {
            return Err("tool parameters must be an object schema".into());
        }
        self.tools.insert(name.into(), (definition, tool));
        Ok(())
    }
    pub fn unregister(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }
    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|(d, _)| d.clone()).collect()
    }
    pub async fn execute(
        &self,
        name: &str,
        args: &Value,
        session: &mut ToolSession,
    ) -> Result<Value, String> {
        if !args.is_object() {
            return Err("tool arguments must be an object".into());
        }
        self.tools
            .get(name)
            .ok_or_else(|| format!("unknown or disabled tool: {name}"))?
            .1
            .execute(args, session)
            .await
    }
}
