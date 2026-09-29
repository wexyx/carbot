use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct AgentList {
    context: Arc<Context>,
}
#[tool(scope="management",name="agent_list",description="List configured local and virtual Agents plus read-only connected remote Agents; use their path when creating projects",parameters=json!({"type":"object","properties":{},"required":[],"additionalProperties":false}))]
impl AgentList {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, _input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let result = self
            .context
            .core()
            .agent_candidates(self.context.project())
            .await?;
        Ok(result["agents"].clone())
    }
}
