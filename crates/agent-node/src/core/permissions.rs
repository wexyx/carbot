use super::Core;
use agent_runtime::permissions::PermissionMode;
use serde_json::{Value, json};
use uuid::Uuid;

/// Human-facing application service. Deliberately not exposed through management tools/A2A control.
impl Core {
    pub(crate) async fn permissions(&self, project: Uuid, id: &str) -> Result<Value, String> {
        self.project(project).await?;
        let row = self
            .state()
            .store
            .get("local_agents", &format!("{project}:{id}"))
            .await
            .ok_or("只能在所属节点配置本地 Agent 权限")?;
        let mode = PermissionMode::parse(row["permission_mode"].as_str().unwrap_or("ask"))?;
        Ok(
            json!({"id":id,"mode":mode,"label":mode.label(),"version":row["version"].as_u64().unwrap_or(0),"scope":"business","effective":"next_task"}),
        )
    }
    pub(crate) async fn set_permissions(
        &self,
        project: Uuid,
        id: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.project(project).await?;
        let mode = input
            .get("mode")
            .map(|v| PermissionMode::parse(v.as_str().unwrap_or("")))
            .transpose()?;
        if input.get("command_allowlist").is_some() {
            return Err("白名单已移到实例全局配置".into());
        }
        if mode.is_none() {
            return Err("mode required".into());
        }
        if mode == Some(PermissionMode::Full) && input["confirm_full_access"] != true {
            return Err("完全访问将取消执行确认，可读写当前系统用户有权访问的文件并联网；必须明确确认。运行中任务不会被取消。".into());
        }
        let expected = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let _guard = self.lifecycle_lock().lock().await;
        self.state().store.transaction(|d|{
            let key=format!("{project}:{id}");
            let mut row=d.get("local_agents",&key).cloned().ok_or("只能配置本地 Agent 权限")?;
            if row["version"].as_u64().unwrap_or(0)!=expected { return Err("version_conflict".into()); }
            let old=row["permission_mode"].clone();
            if let Some(mode)=mode { row["permission_mode"]=json!(mode); }
            row["version"]=json!(expected.checked_add(1).ok_or("version overflow")?);
            d.set("local_agents",&key,row);
            d.set("permission_audit",&Uuid::new_v4().to_string(),json!({"project_id":project,"agent_id":id,"previous":old,"mode":mode,"timestamp":crate::storage::now(),"source":"human"}));
            Ok(())
        }).await?;
        self.permissions(project, id).await
    }
}
