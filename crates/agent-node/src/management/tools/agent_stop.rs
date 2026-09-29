use super::super::context::Context;
use super::arguments::required;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct AgentStop {
    context: Arc<Context>,
}
#[tool(scope="management",name="agent_stop",description="Stop a local business agent; requires explicit human command, never AI self-confirmation",parameters=json!({"type":"object","properties":{"client_id":{"type":"string"}},"required":["client_id"],"additionalProperties":false}))]
impl AgentStop {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .propose_stop(self.context.project(), required(input, "client_id")?)
            .await
    }
}
