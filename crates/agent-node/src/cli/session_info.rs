use crate::management::Manager;
use uuid::Uuid;

/// Minimal footer snapshot; model and context details remain in configuration.
pub(super) struct SessionInfo;
impl SessionInfo {
    pub(super) async fn load(
        manager: &Manager,
        project: Uuid,
        group: Option<&str>,
    ) -> Result<String, String> {
        let Some(group) = group else {
            return Ok("管理 · 请求批准".into());
        };
        let doc = manager
            .core()
            .state()
            .policy_store
            .get(project, "group", group)
            .await?;
        let name = doc.body["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("未命名项目");
        let mut labels = std::collections::BTreeSet::new();
        for member in doc.body["policy"]["members"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let path = member["path"].as_array();
            if path.is_none_or(|p| p.len() != 1) {
                labels.insert("远端管理");
                continue;
            }
            let id = path.unwrap()[0].as_str().unwrap_or_default();
            let row = manager
                .core()
                .state()
                .store
                .get("local_agents", &format!("{project}:{id}"))
                .await;
            let label = row
                .as_ref()
                .map(|row| {
                    agent_runtime::permissions::PermissionMode::parse(
                        row["permission_mode"].as_str().unwrap_or("ask"),
                    )
                    .unwrap_or_default()
                    .label()
                })
                .unwrap_or("按成员配置");
            labels.insert(label);
        }
        let permission = if labels.len() == 1 {
            *labels.first().unwrap()
        } else {
            "按成员配置"
        };
        Ok(format!(
            "{name} #{} · {permission}",
            group.chars().take(8).collect::<String>()
        ))
    }
}
