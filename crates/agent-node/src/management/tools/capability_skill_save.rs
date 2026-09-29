use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct CapabilitySkillSave {
    context: Arc<Context>,
}
#[tool(scope="management",name="capability_skill_save",description="Save a skill package within one capability scope; no Python permission escalation",parameters=json!({"type":"object","properties":{"scope":{"type":"string","enum":["management","business"]},"definition":{"type":"object"},"expected_version":{"type":"integer"}},"required":["scope","definition","expected_version"],"additionalProperties":false}))]
impl CapabilitySkillSave {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        match input["scope"].as_str() {
            Some("management") => {
                super::super::skills::save(
                    self.context.core(),
                    self.context.project(),
                    input.clone(),
                )
                .await
            }
            Some("business") => {
                self.context
                    .core()
                    .save_skill(self.context.project(), input.clone())
                    .await
            }
            _ => Err("invalid skill scope".into()),
        }
    }
}
