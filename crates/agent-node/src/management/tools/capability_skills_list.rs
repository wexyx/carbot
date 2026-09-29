use super::super::context::Context;
use agent_runtime::tools::{ToolContext, ToolSession, tool};
use serde_json::{Value, json};
use std::sync::Arc;
struct CapabilitySkillsList {
    context: Arc<Context>,
}
#[tool(scope="management",name="capability_skills_list",description="List skill packages and revisions separately for management or business",parameters=json!({"type":"object","properties":{"scope":{"type":"string","enum":["management","business"]}},"required":["scope"],"additionalProperties":false}))]
impl CapabilitySkillsList {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        Some(Self {
            context: context.extension::<Context>()?,
        })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let collection = match input["scope"].as_str() {
            Some("management") => "management_skills",
            Some("business") => "skills",
            _ => return Err("invalid skill scope".into()),
        };
        Ok(json!(
            self.context
                .core()
                .state()
                .store
                .list(collection)
                .await
                .into_iter()
                .filter(|r| r["project_id"] == json!(self.context.project()))
                .collect::<Vec<_>>()
        ))
    }
}
