use crate::{
    AppState,
    capabilities::{Context, Library},
};
use agent_runtime::tools::ToolPolicy;
use uuid::Uuid;
pub(crate) async fn load(
    state: &AppState,
    project: Uuid,
    scope: &str,
    agent: &str,
) -> Result<ToolPolicy, String> {
    for_group(state, project, scope, agent, None).await
}
pub(crate) async fn for_group(
    state: &AppState,
    project: Uuid,
    scope: &str,
    agent: &str,
    group: Option<&str>,
) -> Result<ToolPolicy, String> {
    Library::new(state.store.clone())
        .tool_policy(scope, &Context::new(project, agent).in_group(group))
        .await
}
