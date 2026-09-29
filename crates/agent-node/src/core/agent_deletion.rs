use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn delete_agent(
        &self,
        p: Uuid,
        id: &str,
        expected: u64,
    ) -> Result<Value, String> {
        self.project(p).await?;
        let _guard = self.lifecycle_lock().lock().await;
        if id == "default" {
            return Err("默认 Agent 不能删除，请修改它的配置".into());
        }
        if self
            .state()
            .clients
            .lock()
            .await
            .contains_key(&(p, id.into()))
        {
            return Err("请先停止 Agent 再删除".into());
        }
        for kind in ["group", "virtual_agent"] {
            for row in self.state().policy_store.list(p, kind).await? {
                if row.body["kind"] == "agent_test" {
                    continue;
                }
                if row.body["policy"]["members"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .any(|m| m["path"] == json!([id]))
                {
                    return Err(format!("Agent 被 {} 引用，请先移除成员", row.body["name"]));
                }
            }
        }
        let local_key = format!("{p}:{id}");
        let virtual_collection = format!("documents:{p}:virtual_agent");
        self.state()
            .store
            .transaction(|d| {
                let (collection, key) = if d.get("local_agents", &local_key).is_some() {
                    ("local_agents", local_key.as_str())
                } else {
                    (virtual_collection.as_str(), id)
                };
                let row = d.get(collection, key).ok_or("not a local Agent")?;
                if row["version"].as_u64().unwrap_or(0) != expected {
                    return Err("version_conflict".into());
                }
                if d.list("runs").iter().any(|r| {
                    r["project_id"] == json!(p)
                        && matches!(r["status"].as_str(), Some("running" | "queued"))
                }) {
                    return Err("请等待当前任务完成后删除 Agent".into());
                }
                if let Some(rows) = d.collections.get_mut(collection) {
                    rows.remove(key);
                }
                if let Some(rows) = d.collections.get_mut("credentials") {
                    rows.retain(|_, r| !(r["project_id"] == json!(p) && r["client_id"] == id));
                }
                Ok(json!({"status":"deleted","history_preserved":true}))
            })
            .await
    }
}
