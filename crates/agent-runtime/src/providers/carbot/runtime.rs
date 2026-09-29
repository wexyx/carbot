use super::config::HarnessConfig;
use super::engine::Engine;
use crate::{EventSink, RuntimeFuture, RuntimeKind};

pub(crate) struct Runtime {
    engine: Engine,
    catalog: serde_json::Value,
}

impl Runtime {
    pub(crate) fn new(
        config: HarnessConfig,
        tools: crate::tools::ToolRegistry,
        catalog: serde_json::Value,
    ) -> Result<Self, String> {
        Ok(Self {
            engine: Engine::new(config, tools)?,
            catalog,
        })
    }
}

impl crate::providers::Provider for Runtime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Carbot
    }
    fn handles_tools(&self) -> bool {
        true
    }
    fn execute<'a>(
        &'a self,
        prompt: &'a str,
        on_event: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a> {
        Box::pin(async move {
            let prompt = format!(
                "Available skill catalog (metadata): {}\n{prompt}",
                self.catalog
            );
            self.engine.run(&prompt, &mut |event| on_event(event)).await
        })
    }
}
