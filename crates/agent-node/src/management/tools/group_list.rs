use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct GroupList {
    context: Arc<Context>,
}
#[tool(scope="management",name="group_list",description="List group policies",parameters=json!({"type":"object","properties":{},"required":[],"additionalProperties":false}))]
impl GroupList {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "group.list", input.clone())
            .await
    }
}
