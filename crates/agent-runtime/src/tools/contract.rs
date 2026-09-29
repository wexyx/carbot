use serde::Serialize;
use serde_json::Value;
use std::{collections::HashSet, future::Future, pin::Pin};
pub type ToolFuture<'a> = Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
#[derive(Clone, Serialize)]
pub struct ToolDefinition {
    name: String,
    description: String,
    parameters: Value,
}
impl ToolDefinition {
    pub fn new(name: impl Into<String>, description: impl Into<String>, parameters: Value) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn parameters(&self) -> &Value {
        &self.parameters
    }
}
#[derive(Default)]
pub struct ToolSession {
    loaded_skills: HashSet<String>,
}
impl ToolSession {
    pub(crate) fn loaded(&self, id: &str) -> bool {
        self.loaded_skills.contains(id)
    }
    pub(crate) fn mark_loaded(&mut self, id: &str) {
        self.loaded_skills.insert(id.into());
    }
}
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn execute<'a>(&'a self, args: &'a Value, session: &'a mut ToolSession) -> ToolFuture<'a>;
}
