/// Private provider contract: providers return an outcome; the public adapter owns termination.
pub(crate) trait Provider: Send + Sync {
    fn kind(&self) -> crate::RuntimeKind;
    /// Native harnesses own tool execution; CLI providers need the shared JSON bridge.
    fn handles_tools(&self) -> bool {
        false
    }
    fn execute<'a>(
        &'a self,
        prompt: &'a str,
        events: &'a mut crate::EventSink<'_>,
    ) -> crate::RuntimeFuture<'a>;
}
