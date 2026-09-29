use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct TemplateGet {
    context: Arc<Context>,
}
#[tool(scope="management",name="template_get",description="Read this node's default group policy template",parameters=json!({"type":"object","properties":{},"required":[],"additionalProperties":false}))]
impl TemplateGet {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "template.get", input.clone())
            .await
    }
}
