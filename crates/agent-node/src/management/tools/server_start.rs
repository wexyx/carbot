use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct ServerStart {
    context: Arc<Context>,
}
#[tool(scope="management",name="server_start",description="Start Server on loopback. Omit address/port to reuse startup or saved configuration (default 8787). Only pass port 0 when the user explicitly requests a random port",parameters=json!({"type":"object","properties":{"address":{"type":"string"},"port":{"type":"integer","minimum":0,"maximum":65535}},"required":[],"additionalProperties":false}))]
impl ServerStart {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context.manager()?.start_server(input.clone()).await
    }
}
