use super::super::{config::HarnessConfig, turn::Turn};
use crate::tools::{ToolDefinition, ToolRegistry};
use serde_json::Value;
use std::collections::BTreeMap;
pub(in super::super) trait ModelProtocol: Send + Sync {
    fn tool(&self, definition: &ToolDefinition) -> Value;
    fn request(
        &self,
        http: &reqwest::Client,
        config: &HarnessConfig,
        history: &[Value],
        tools: &ToolRegistry,
    ) -> reqwest::RequestBuilder;
    fn consume(
        &self,
        turn: &mut Turn,
        value: Value,
        delta: &mut dyn FnMut(String),
    ) -> Result<(), String>;
    fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    );
    fn tools(&self, registry: &ToolRegistry) -> Vec<Value> {
        registry
            .definitions()
            .iter()
            .map(|d| self.tool(d))
            .collect()
    }
}
