use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct SubtreePolicyGet {
    context: Arc<Context>,
}
#[tool(scope="management",name="subtree_policy_get",description="Read a descendant group's copied subgroup policy",parameters=json!({"type":"object","properties":{"path":{"type":"array","items":{"type":"string"}},"key":{"type":"string"}},"required":["path","key"],"additionalProperties":false}))]
impl SubtreePolicyGet {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .subtree_policy(self.context.project(), "subgroup.get", input.clone())
            .await
    }
}
