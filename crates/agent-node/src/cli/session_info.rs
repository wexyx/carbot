use crate::management::Manager;
use serde_json::Value;
use uuid::Uuid;

/// Display-only snapshot, refreshed on navigation/configuration, never on every frame.
pub(super) struct SessionInfo;
impl SessionInfo {
    pub(super) async fn load(
        manager: &Manager,
        project: Uuid,
        group: Option<&str>,
    ) -> Result<String, String> {
        let mut settings = manager.configuration().await?;
        let mut permission = agent_runtime::permissions::PermissionMode::Ask;
        let (name, agent) = if let Some(group) = group {
            let doc = manager
                .core()
                .state()
                .policy_store
                .get(project, "group", group)
                .await?;
            let name = doc.body["name"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or("未命名项目")
                .to_owned();
            let members = doc.body["policy"]["members"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            if members.len() != 1 {
                return Ok(format!(
                    "项目 {name} · {} · {} 位成员 · 上下文按各 Agent 配置",
                    doc.body["policy"]["mode"].as_str().unwrap_or("群聊"),
                    members.len()
                ));
            }
            let path = members[0]["path"].as_array().cloned().unwrap_or_default();
            if path.len() != 1 {
                return Ok(format!("项目 {name} · 远端 Agent · 上下文由远端管理"));
            }
            let id = path[0].as_str().unwrap_or_default();
            let row = manager
                .core()
                .state()
                .store
                .get("local_agents", &format!("{project}:{id}"))
                .await;
            let Some(row) = row else {
                return Ok(format!("项目 {name} · {id} · 组合/远端 Agent"));
            };
            if let Some(values) = row.get("configuration").filter(|v| v.is_object()) {
                settings
                    .update(serde_json::from_value(values.clone()).map_err(|e| e.to_string())?)?;
            }
            permission = agent_runtime::permissions::PermissionMode::parse(
                row["permission_mode"].as_str().unwrap_or("ask"),
            )
            .unwrap_or_default();
            settings.set(
                "ADMIN_AGENT_PROVIDER",
                row["provider"].as_str().unwrap_or("mock").into(),
            );
            (format!("项目 {name}"), id.to_owned())
        } else {
            ("管理".into(), "默认 Agent".into())
        };
        Ok(Self::format(
            &name,
            &agent,
            &settings.public_view()["values"],
            permission,
        ))
    }
    fn format(
        name: &str,
        agent: &str,
        values: &Value,
        permission: agent_runtime::permissions::PermissionMode,
    ) -> String {
        let provider = values["ADMIN_AGENT_PROVIDER"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("carbot");
        let permission = permission.label();
        if provider != "carbot" {
            return format!("{name} · {agent} · {provider} · {permission} · 上下文由运行器管理");
        }
        let model = values["MODEL_NAME"].as_str().unwrap_or("未配置模型");
        let budget = values["CONTEXT_MAX_TOKENS"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or("65536");
        let strategy = match values["CONTEXT_STRATEGY"].as_str().unwrap_or("extractive") {
            "window" => "近期窗口",
            "disabled" => "不压缩",
            _ => "首尾摘录",
        };
        format!(
            "{name} · {agent}/{model} · {permission} · 上下文上限 {budget} token（估算）· {strategy}"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn displays_budget_not_invented_usage_and_never_secrets() {
        let text = SessionInfo::format(
            "项目 demo",
            "agent",
            &serde_json::json!({"MODEL_NAME":"model","MODEL_API_KEY":"secret","CONTEXT_MAX_TOKENS":"32768","CONTEXT_STRATEGY":"window"}),
            agent_runtime::permissions::PermissionMode::Ask,
        );
        assert!(text.contains("32768"));
        assert!(text.contains("近期窗口"));
        assert!(!text.contains("secret"));
        assert!(!text.contains('%'));
    }
}
