use super::profile::Profile;
use crate::skills::ExecutionRequest;
use serde_json::Value;
use std::{future::Future, pin::Pin};
pub(crate) type ExecutionFuture<'a> =
    Pin<Box<dyn Future<Output = Result<Value, String>> + Send + 'a>>;
pub(crate) trait ProcessExecutor: Send + Sync {
    fn execute<'a>(
        &'a self,
        request: &'a ExecutionRequest,
        profile: &'a Profile,
    ) -> ExecutionFuture<'a>;
}
