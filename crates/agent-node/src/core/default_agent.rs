use super::Core;
use serde_json::json;

impl Core {
    pub(super) async fn save_default_role(
        &self,
        project: uuid::Uuid,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let _guard = self.lifecycle_lock().lock().await;
        let key = format!("{project}:default");
        self.state()
            .store
            .transaction(|data| {
                let mut row = data
                    .get("local_agents", &key)
                    .cloned()
                    .ok_or("default Agent missing")?;
                if row["version"] != input["expected_version"] {
                    return Err("version_conflict".into());
                }
                if row["provider"] != input["provider"] {
                    return Err("默认 Agent 的运行器请通过模型配置修改".into());
                }
                row["response_instructions"] =
                    json!(super::response_instructions::resolve(&input, Some(&row))?);
                row["role"] = input["role"].clone();
                row["version"] = json!(
                    row["version"]
                        .as_u64()
                        .unwrap_or(0)
                        .checked_add(1)
                        .ok_or("version overflow")?
                );
                data.set("local_agents", &key, row.clone());
                Ok(row)
            })
            .await
    }

    pub(crate) async fn default_agent(
        &self,
        config: agent_runtime::config::RuntimeConfig,
        persist: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let provider = config.kind().as_str();
        let p = self.bootstrap().await?;
        let _guard = self.lifecycle_lock().lock().await;
        let key = format!("{p}:default");
        if self.state().store.list("runs").await.iter().any(|r| {
            r["project_id"] == json!(p)
                && matches!(r["status"].as_str(), Some("running" | "queued"))
        }) {
            return Err("请等待项目任务完成后修改默认 Agent".into());
        }
        if self
            .state()
            .store
            .get("local_agents", &key)
            .await
            .is_some_and(|r| r["is_default"] != true)
        {
            return Err("Agent ID default is already used".into());
        }
        // Persist human configuration before replacing the running default provider.
        persist()?;
        self.state().store.transaction(|d|{
            if d.get("local_agents",&key).is_some_and(|r|r["is_default"]!=true) {
                return Err("Agent ID default is already used; rename it before enabling the default Agent".into());
            }
            let role = d.get("local_agents",&key).and_then(|r|r["role"].as_str()).unwrap_or("默认 Agent").to_owned();
            let version = d.get("local_agents",&key).and_then(|r|r["version"].as_u64()).unwrap_or(0).checked_add(1).ok_or("version overflow")?;
            let permission=d.get("local_agents",&key).and_then(|r|r["permission_mode"].as_str()).unwrap_or("ask").to_owned();
            let response_instructions=d.get("local_agents",&key).and_then(|r|r["response_instructions"].as_str()).unwrap_or(super::response_instructions::DEFAULT).to_owned();
            d.set("local_agents",&key,json!({"response_instructions":response_instructions,"permission_mode":permission,"project_id":p,"client_id":"default","provider":provider,"role":role,"is_default":true,"enabled":true,"version":version}));
            Ok(())
        }).await?;
        let child = serde_json::from_value(
            json!({"client_id":"default","role":"默认 Agent","provider":provider}),
        )
        .map_err(|e| e.to_string())?;
        crate::local::start_configured(self.state().clone(), p, child, Some(config)).await;
        Ok(())
    }
}
