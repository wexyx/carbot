use super::Core;
use agent_runtime::permissions::CommandAllowlist;
use serde_json::{Value, json};
impl Core {
    pub(crate) async fn command_allowlist(&self) -> Value {
        let row = self
            .state()
            .store
            .get("settings", "command_allowlist")
            .await
            .unwrap_or(Value::Null);
        json!({"scope":"instance","version":row["version"].as_u64().unwrap_or(0),"command_allowlist":row.get("commands").cloned().unwrap_or_else(||json!(CommandAllowlist::default().commands())),"default_allowlist":CommandAllowlist::default().commands()})
    }
    pub(crate) async fn set_command_allowlist(&self, input: Value) -> Result<Value, String> {
        let list = CommandAllowlist::new(
            serde_json::from_value(input["command_allowlist"].clone())
                .map_err(|e| e.to_string())?,
        )?;
        let expected = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let _guard = self.lifecycle_lock().lock().await;
        self.state().store.transaction(|d|{
            let old=d.get("settings","command_allowlist").cloned().unwrap_or(Value::Null);
            if old["version"].as_u64().unwrap_or(0)!=expected{return Err("version_conflict".into());}
            d.set("settings","command_allowlist",json!({"version":expected.checked_add(1).ok_or("version overflow")?,"commands":list.commands()}));
            d.set("permission_audit",&uuid::Uuid::new_v4().to_string(),json!({"source":"human","scope":"instance","previous":old,"commands":list.commands(),"timestamp":crate::storage::now()}));
            Ok(())
        }).await?;
        Ok(self.command_allowlist().await)
    }
}
