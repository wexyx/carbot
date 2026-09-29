use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;
impl Core {
    pub(crate) async fn propose_stop(
        &self,
        project: Uuid,
        client_id: &str,
    ) -> Result<Value, String> {
        self.project(project).await?;
        if self
            .state()
            .store
            .get("local_agents", &format!("{project}:{client_id}"))
            .await
            .is_none()
        {
            return Err("not a local agent".into());
        }
        let id = Uuid::new_v4();
        let row = json!({"id":id,"project_id":project,"tool":"agent_stop","client_id":client_id,"status":"pending","expires_at":crate::storage::now()+300});
        self.state()
            .store
            .insert("management_approvals", &id.to_string(), row.clone())
            .await?;
        Ok(row)
    }
    pub(crate) async fn approvals(&self, project: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        Ok(json!(
            self.state()
                .store
                .list("management_approvals")
                .await
                .into_iter()
                .filter(|r| r["project_id"] == json!(project)
                    && r["status"] == "pending"
                    && r["expires_at"].as_u64().unwrap_or(0) > crate::storage::now())
                .map(redacted)
                .collect::<Vec<_>>()
        ))
    }
    pub(crate) async fn propose_mount(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        if let Some(row) = self.existing_connection(project, &input).await? {
            return Ok(row);
        }
        if let Some(row) = self
            .approvals(project)
            .await?
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["tool"] == "peer_mount" && r["input"]["url"] == input["url"])
        {
            return Ok(row.clone());
        }
        let id = Uuid::new_v4();
        let row = json!({"id":id,"project_id":project,"tool":"peer_mount","input":input,"status":"pending","expires_at":crate::storage::now()+300,
            "warning":"连接会授予上游发现、调用本节点 Agent 以及管理后代子树群策略的能力；Agent 定义和模型配置只能在本机修改。"});
        self.state()
            .store
            .insert("management_approvals", &id.to_string(), row.clone())
            .await?;
        Ok(redacted(row))
    }
    pub(crate) async fn decide(
        &self,
        project: Uuid,
        id: Uuid,
        allow: bool,
    ) -> Result<Value, String> {
        self.project(project).await?;
        let row = self
            .state()
            .store
            .transaction(|data| {
                let mut row = data
                    .get("management_approvals", &id.to_string())
                    .cloned()
                    .ok_or("not_found")?;
                if row["project_id"] != json!(project) {
                    return Err("forbidden".into());
                }
                if row["status"] != "pending"
                    || row["expires_at"].as_u64().unwrap_or(0) <= crate::storage::now()
                {
                    return Err("approval expired or already resolved".into());
                }
                row["status"] = json!(if allow { "executing" } else { "denied" });
                data.set("management_approvals", &id.to_string(), row.clone());
                Ok(row)
            })
            .await?;
        if !allow {
            return Ok(redacted(row));
        }
        let result = match row["tool"].as_str() {
            Some("peer_mount") => self.mount(project, row["input"].clone()).await,
            Some("agent_stop") => {
                self.agent_stop(
                    project,
                    row["client_id"].as_str().ok_or("invalid pending action")?,
                )
                .await
            }
            _ => Err("unknown pending action".into()),
        };
        self.state()
            .store
            .transaction(|data| {
                let mut row = row;
                row["status"] = json!(if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                });
                row["result"] = json!(result.as_ref().ok());
                row["error"] = json!(result.as_ref().err());
                data.set("management_approvals", &id.to_string(), row);
                Ok(())
            })
            .await?;
        result
    }
}
fn redacted(mut row: Value) -> Value {
    if let Some(input) = row["input"].as_object_mut() {
        for key in ["enrollment_token", "ak", "sk"] {
            if input.contains_key(key) {
                input.insert(key.into(), json!("[redacted]"));
            }
        }
    }
    row
}
