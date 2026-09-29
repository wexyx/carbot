//! Public runtime interface and callback types.
use crate::{RuntimeEvent, RuntimeKind};

pub type DeltaSink<'a> = dyn FnMut(String) + Send + 'a;
pub type EventSink<'a> = dyn FnMut(RuntimeEvent) + Send + 'a;
pub type RuntimeFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;

/// A reusable execution provider. Each run owns its own conversation state.
pub trait AgentRuntime: Send + Sync {
    fn kind(&self) -> RuntimeKind;
    fn run_events<'a>(
        &'a self,
        prompt: &'a str,
        events: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a>;
    fn run<'a>(&'a self, prompt: &'a str, on_delta: &'a mut DeltaSink<'_>) -> RuntimeFuture<'a> {
        Box::pin(async move {
            self.run_events(prompt, &mut |event| {
                if let Some(text) = event.legacy_delta() {
                    on_delta(text);
                }
            })
            .await
        })
    }
}
