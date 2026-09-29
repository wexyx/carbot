use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct SubtreePolicyUpdate {
    context: Arc<Context>,
}
#[tool(scope="management",name="subtree_policy_update",description="Update a copied descendant subgroup policy using its current version",parameters=json!({"type":"object","properties":{"path":{"type":"array","items":{"type":"string"}},"key":{"type":"string"},"expected_version":{"type":"integer"},"policy":{"type":"object"}},"required":["path","key","expected_version","policy"],"additionalProperties":false}))]
impl SubtreePolicyUpdate {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        self.context
            .core()
            .subtree_policy(self.context.project(), "subgroup.update", input.clone())
            .await
    }
}
