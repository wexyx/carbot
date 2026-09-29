use super::{ExternalCommand, Tool, ToolContext, ToolDefinition, ToolFuture, ToolSession};
use serde_json::{Value, json};
use std::sync::Arc;
pub(super) struct CommandTool {
    definition: ExternalCommand,
    context: Arc<ToolContext>,
}
impl CommandTool {
    pub(super) fn new(definition: ExternalCommand, context: Arc<ToolContext>) -> Self {
        Self {
            definition,
            context,
        }
    }
}
impl Tool for CommandTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition::new(
            self.definition.name(),
            format!(
                "{} — requires one-shot human approval; runs a fixed shell command on the host (including access outside the workspace)",
                self.definition.description()
            ),
            json!({"type":"object","properties":{},"additionalProperties":false}),
        )
    }
    fn execute<'a>(&'a self, args: &'a Value, _session: &'a mut ToolSession) -> ToolFuture<'a> {
        Box::pin(async move {
            if args.as_object().is_none_or(|o| !o.is_empty()) {
                return Err(
                    "external command takes no arguments; command text is host-configured".into(),
                );
            }
            super::shell_execution::run(&self.context, self.definition.command()).await
        })
    }
}
