use super::{ToolContext, ToolSession, skill_args::SkillArgs};
use serde_json::{Value, json};
use std::sync::Arc;
struct SkillFile {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(scope = "shared", name = "skill_file", description = "Read a registered skill resource.", parameters = json!({"type":"object","properties":{"skill_id":{"type":"string"},"path":{"type":"string"},"args":{"type":"array","items":{"type":"string"}}},"required":["skill_id"],"additionalProperties":false}), runtime = crate)]
impl SkillFile {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        (!context.skills().is_empty()).then_some(Self { context })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let args: SkillArgs = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        let skill = self.context.catalog.get(&args.skill_id)?;
        Ok(
            json!({"path":args.path,"content":skill.files().get(&args.path).ok_or("unknown skill file")?}),
        )
    }
}
