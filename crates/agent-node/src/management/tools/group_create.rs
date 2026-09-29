use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct GroupCreate {
    context: Arc<Context>,
}
#[tool(scope="management",name="group_create",description="Create a project/chat with a validated policy. chat mode requires exactly one configured Agent. Name is optional; first message provides its title",parameters=json!({"type":"object","properties":{"name":{"type":"string"},"policy":{"type":"object"}},"required":["policy"],"additionalProperties":false}))]
impl GroupCreate {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "group.create", input.clone())
            .await
    }
}
