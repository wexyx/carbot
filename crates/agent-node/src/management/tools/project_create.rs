use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct ProjectCreate {
    context: Arc<Context>,
}
#[tool(scope="management",name="project_create",description="Create a new isolated space and project; conversations remain bound to their original project",parameters=json!({"type":"object","properties":{"name":{"type":"string"},"space_name":{"type":"string"}},"required":["name","space_name"],"additionalProperties":false}))]
impl ProjectCreate {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context.core().create_project(input.clone()).await
    }
}
