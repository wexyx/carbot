use super::{ToolContext, ToolSession, skill_args::SkillArgs};
use serde_json::{Value, json};
use std::sync::Arc;
struct SkillRead {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(scope = "shared", name = "skill_read", description = "Read SKILL.md before executing scripts.", parameters = json!({"type":"object","properties":{"skill_id":{"type":"string"},"path":{"type":"string"},"args":{"type":"array","items":{"type":"string"}}},"required":["skill_id"],"additionalProperties":false}), runtime = crate)]
impl SkillRead {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        (!context.skills().is_empty()).then_some(Self { context })
    }
    async fn execute(&self, input: &Value, session: &mut ToolSession) -> Result<Value, String> {
        let args: SkillArgs = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        let skill = self.context.catalog.get(&args.skill_id)?;
        session.mark_loaded(skill.id());
        Ok(
            json!({"skill_id":skill.id(),"instructions":skill.files()["SKILL.md"],"files":skill.files().keys().collect::<Vec<_>>()}),
        )
    }
}
