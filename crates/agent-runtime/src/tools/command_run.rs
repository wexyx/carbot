use super::{ToolContext, ToolSession};
use serde::Deserialize;
use serde_json::{Value, json};
use std::sync::Arc;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Input {
    command: String,
}
struct CommandRun {
    context: Arc<ToolContext>,
}
#[crate::tools::tool(name="command_run",description="Run a shell command in this Agent workspace sandbox. Authorization follows the host-selected permission mode; never approve your own actions. May modify/delete workspace files or access the network according to the host profile. Never bypass an approval or sandbox denial.",parameters=json!({"type":"object","properties":{"command":{"type":"string","maxLength":8192}},"required":["command"],"additionalProperties":false}),runtime=crate)]
impl CommandRun {
    fn new(context: Arc<ToolContext>) -> Option<Self> {
        context.workdir().is_some().then_some(Self { context })
    }
    async fn execute(&self, input: &Value, _session: &mut ToolSession) -> Result<Value, String> {
        let input: Input = serde_json::from_value(input.clone()).map_err(|e| e.to_string())?;
        super::shell_execution::run(&self.context, &input.command).await
    }
}
