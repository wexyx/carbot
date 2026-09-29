use super::super::context::Context;
use super::arguments::required;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct SessionHistory {
    context: Arc<Context>,
}
#[tool(scope="management",name="session_history",description="Read business conversation events, limited to the selected project",parameters=json!({"type":"object","properties":{"session_id":{"type":"string"}},"required":["session_id"],"additionalProperties":false}))]
impl SessionHistory {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .history(
                self.context.project(),
                required(input, "session_id")?
                    .parse()
                    .map_err(|_| "invalid session_id")?,
            )
            .await
    }
}
