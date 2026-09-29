use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct TemplateUpdate {
    context: Arc<Context>,
}
#[tool(scope="management",name="template_update",description="Update default policy template without changing existing copied groups",parameters=json!({"type":"object","properties":{"expected_version":{"type":"integer"},"policy":{"type":"object"}},"required":["expected_version","policy"],"additionalProperties":false}))]
impl TemplateUpdate {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .control(self.context.project(), "template.update", input.clone())
            .await
    }
}
