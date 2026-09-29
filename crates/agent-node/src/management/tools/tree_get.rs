use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct TreeGet {
    context: Arc<Context>,
}
#[tool(scope="management",name="tree_get",description="Discover reachable descendants",parameters=json!({"type":"object","properties":{},"required":[],"additionalProperties":false}))]
impl TreeGet {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "tree.get", input.clone())
            .await
    }
}
