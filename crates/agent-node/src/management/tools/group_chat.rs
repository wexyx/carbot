use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct GroupChat {
    context: Arc<Context>,
}
#[tool(scope="management",name="group_chat",description="Send a task to a group; returns session_id for progress/history",parameters=json!({"type":"object","properties":{"group_id":{"type":"string"},"content":{"type":"string"},"previous_session_id":{"type":"string"}},"required":["group_id","content"],"additionalProperties":false}))]
impl GroupChat {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .group_chat(self.context.project(), input.clone())
            .await
    }
}
