//! Propagates a project's capability context through the policy engine, then into worker commands.
tokio::task_local! { static PROJECT: (String, serde_json::Value, uuid::Uuid); }

pub(crate) struct ProjectExecution;
impl ProjectExecution {
    pub(crate) async fn scope<T>(
        project: String,
        run: uuid::Uuid,
        workspace: serde_json::Value,
        work: impl std::future::Future<Output = T>,
    ) -> T {
        PROJECT.scope((project, workspace, run), work).await
    }
    pub(crate) fn run() -> Option<uuid::Uuid> {
        PROJECT.try_with(|value| value.2).ok()
    }
    pub(crate) fn workspace() -> serde_json::Value {
        PROJECT
            .try_with(|value| value.1.clone())
            .unwrap_or(serde_json::Value::Null)
    }
    pub(crate) fn current() -> Option<String> {
        PROJECT.try_with(|value| value.0.clone()).ok()
    }
}
