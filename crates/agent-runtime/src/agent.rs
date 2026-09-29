//! Compatibility functions; new callers can keep an AgentRuntime returned by RuntimeFactory.
use crate::RuntimeFactory;
pub use crate::is_token_insufficient;

pub async fn run(prompt: &str, mut on_delta: impl FnMut(String) + Send) -> Result<String, String> {
    RuntimeFactory::from_env()?.run(prompt, &mut on_delta).await
}
pub async fn run_with_provider(
    provider: &str,
    prompt: &str,
    mut on_delta: impl FnMut(String) + Send,
) -> Result<String, String> {
    RuntimeFactory::create(provider)?
        .run(prompt, &mut on_delta)
        .await
}
