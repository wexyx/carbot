use super::client::ModelClient;
use crate::{
    RuntimeEvent,
    tools::{ToolRegistry, ToolSession},
};
use serde_json::{Value, json};
use std::collections::BTreeMap;
/// Per-task conversation and tool authorization state.
pub(super) struct Run<'a> {
    client: &'a ModelClient,
    tools: &'a ToolRegistry,
    max_steps: usize,
    history: Vec<Value>,
    session: ToolSession,
}
impl<'a> Run<'a> {
    pub(super) fn new(
        client: &'a ModelClient,
        tools: &'a ToolRegistry,
        max_steps: usize,
        prompt: &str,
    ) -> Self {
        Self {
            client,
            tools,
            max_steps,
            history: vec![json!({"role":"user","content":prompt})],
            session: ToolSession::default(),
        }
    }
    pub(super) async fn execute(
        mut self,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<String, String> {
        for _ in 0..self.max_steps {
            self.client
                .prepare_context(&mut self.history, self.tools, events)?;
            let turn = self
                .client
                .next_turn(&self.history, self.tools, events)
                .await?;
            if turn.calls().is_empty() {
                return Ok(turn.text().into());
            }
            let mut results = BTreeMap::new();
            for &index in turn.calls().keys() {
                let (id, name) = turn.call(index)?;
                let args: Value = serde_json::from_str(turn.arguments(index))
                    .map_err(|_| "invalid tool arguments JSON")?;
                events(RuntimeEvent::ToolStarted {
                    id: id.into(),
                    name: name.into(),
                });
                let result = self
                    .tools
                    .execute(name, &args, &mut self.session)
                    .await
                    .map(|v| {
                        v.as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string())
                    })
                    .unwrap_or_else(|e| json!({"error":e}).to_string());
                events(RuntimeEvent::ToolFinished {
                    id: id.into(),
                    output: result.clone(),
                });
                results.insert(index, result);
            }
            self.client
                .append_history(&mut self.history, &turn, &results);
        }
        Err(format!("harness reached its {} step limit", self.max_steps))
    }
}
