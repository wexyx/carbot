use super::{Core, policies::Policy};
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn test_agent(&self, p: Uuid, input: Value) -> Result<Value, String> {
        self.project(p).await?;
        let policy:Policy=serde_json::from_value(json!({"mode":"chat","members":[{"path":input["path"],"role":"测试对话"}],"rounds":1,"instructions":"","leader":null})).map_err(|e|e.to_string())?;
        policy.validate()?;
        super::agent_directory::validate_members(self.state(), p, &policy, None).await?;
        let digest = crate::hash_secret(&format!("{p}:agent-test:{}", input["path"]));
        let key = Uuid::parse_str(&digest[..32])
            .map_err(|e| e.to_string())?
            .to_string();
        if let Ok(row) = self.state().policy_store.get(p, "group", &key).await {
            return Ok(json!(row));
        }
        let result=self.state().policy_store.put(p,"group",&key,0,json!({"name":"Agent 测试","kind":"agent_test","policy":policy,"status":"ready","auto_name":false})).await;
        match result {
            Ok(row) => Ok(json!(row)),
            Err(e) if e == "version_conflict" => Ok(json!(
                self.state().policy_store.get(p, "group", &key).await?
            )),
            Err(e) => Err(e),
        }
    }
}

pub(super) async fn is_test(state: &crate::AppState, project: Uuid) -> bool {
    let Some(group) = super::project_execution::ProjectExecution::current() else {
        return false;
    };
    state
        .policy_store
        .get(project, "group", &group)
        .await
        .is_ok_and(|row| row.body["kind"] == "agent_test")
}
