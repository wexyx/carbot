use super::{ToolContext, ToolSession, skill_args::SkillArgs};
use serde_json::{Value, json};
use std::sync::Arc;
struct PythonRun {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name = "python_run", description = "Execute an approved skill script through the node's sandbox.", parameters = json!({"type":"object","properties":{"skill_id":{"type":"string"},"path":{"type":"string"},"args":{"type":"array","items":{"type":"string"}}},"required":["skill_id"],"additionalProperties":false}), runtime = crate)]
impl PythonRun {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        (!context.skills().is_empty()).then_some(Self { context })
    }
    async fn execute(&self, input: &Value, session: &mut ToolSession) -> Result<Value, String> {
        let args: SkillArgs = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        let skill = self.context.catalog.get(&args.skill_id)?;
        if !session.loaded(skill.id()) {
            return Err("load skill_read before executing its script".into());
        }
        self.context
            .python
            .run(skill, &args.path, &args.args, &args.access)
            .await
    }
}
