use crate::{
    AppState,
    capabilities::{Context, Library},
};
use agent_runtime::skills::SkillCatalog;
use uuid::Uuid;
pub(crate) async fn snapshot(state: &AppState, project: Uuid) -> Result<SkillCatalog, String> {
    for_agent(state, project, "").await
}
pub(crate) async fn for_agent(
    state: &AppState,
    project: Uuid,
    agent: &str,
) -> Result<SkillCatalog, String> {
    for_group(state, project, agent, None).await
}
pub(crate) async fn for_group(
    state: &AppState,
    project: Uuid,
    agent: &str,
    group: Option<&str>,
) -> Result<SkillCatalog, String> {
    Library::new(state.store.clone())
        .skills("business", &Context::new(project, agent).in_group(group))
        .await
}
