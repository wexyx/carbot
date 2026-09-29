use super::{client::ModelClient, config::HarnessConfig, run::Run};
use crate::{RuntimeEvent, tools::ToolRegistry};
use std::time::Duration;
/// Reusable dependencies; invocation state belongs to Run.
pub(super) struct Engine {
    client: ModelClient,
    tools: ToolRegistry,
    max_steps: usize,
}
impl Engine {
    pub(super) fn new(config: HarnessConfig, tools: ToolRegistry) -> Result<Self, String> {
        let config = config.validate()?;
        let max_steps = config.max_steps;
        Ok(Self {
            client: ModelClient::new(config)?,
            tools,
            max_steps,
        })
    }
    pub(super) async fn run(
        &self,
        prompt: &str,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<String, String> {
        tokio::time::timeout(
            Duration::from_secs(600),
            Run::new(&self.client, &self.tools, self.max_steps, prompt).execute(events),
        )
        .await
        .map_err(|_| "harness task exceeded 600 seconds")?
    }
}
