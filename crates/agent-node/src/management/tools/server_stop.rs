use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct ServerStop {
    context: Arc<Context>,
}
#[tool(scope="management",name="server_stop",description="Stop the HTTP Server; business and AdminAgent continue running",parameters=json!({"type":"object","properties":{},"required":[],"additionalProperties":false}))]
impl ServerStop {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, _input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let manager = self.context.manager()?;
        manager.web().stop().await;
        Ok(json!({"web":"stopped","agents":"running"}))
    }
}
