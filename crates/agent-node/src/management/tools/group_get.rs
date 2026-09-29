use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct GroupGet {
    context: Arc<Context>,
}
#[tool(scope="management",name="group_get",description="Read a group, policy, version and subtree bindings",parameters=json!({"type":"object","properties":{"key":{"type":"string"}},"required":["key"],"additionalProperties":false}))]
impl GroupGet {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "group.get", input.clone())
            .await
    }
}
