use super::Manager;
use agent_runtime::{
    skills::ExecutionPolicy,
    tools::{ToolContext, ToolFactory},
};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
impl Manager {
    pub(crate) async fn tool_catalog(self: &Arc<Self>, project: Uuid) -> Result<Value, String> {
        self.core().project(project).await?;
        let mut management = self.base_registry(project).await?.definitions();
        let root = agent_runtime::paths::workdir();
        let context = ToolContext::new(
            Some(root),
            agent_runtime::skills::SkillCatalog::new(vec![
                agent_runtime::skills::SkillDefinition::new(
                    "catalog-placeholder".into(),
                    "Schema discovery only".into(),
                    std::collections::BTreeMap::from([(
                        "SKILL.md".into(),
                        "Schema discovery only".into(),
                    )]),
                    true,
                    false,
                )?,
            ])?,
            ExecutionPolicy::from_env()?,
        )?;
        let business = ToolFactory::create(context)?.definitions();
        for item in &business {
            if matches!(item.name(), "skill_read" | "skill_file")
                && !management.iter().any(|d| d.name() == item.name())
            {
                management.push(item.clone());
            }
        }
        let agents = self
            .core()
            .state()
            .store
            .list("local_agents")
            .await
            .into_iter()
            .filter(|r| r["project_id"] == json!(project))
            .map(|r| json!({"id":r["client_id"],"provider":r["provider"]}))
            .collect::<Vec<_>>();
        Ok(
            json!({"agents":agents,"management":management,"business":business,"note":"本节点注册工具。实际可执行性仍受 Provider、Skill 和沙箱权限限制；不包含远端节点或 Codex/Claude CLI 自带的私有工具。"}),
        )
    }
}
