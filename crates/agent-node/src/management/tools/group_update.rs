use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct GroupUpdate {
    context: Arc<Context>,
}
#[tool(scope="management",name="group_update",description="Change group members, mode or policy with optimistic version checking",parameters=json!({"type":"object","properties":{"key":{"type":"string"},"expected_version":{"type":"integer"},"policy":{"type":"object"}},"required":["key","expected_version","policy"],"additionalProperties":false}))]
impl GroupUpdate {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "group.update", input.clone())
            .await
    }
}
