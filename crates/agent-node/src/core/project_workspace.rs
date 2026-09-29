use crate::AppState;
use agent_runtime::workspace::WorkspaceSettings;
use serde_json::{Value, json};

impl super::Core {
    pub(crate) async fn project_workspace(
        &self,
        project: uuid::Uuid,
        group: Option<&str>,
    ) -> Result<WorkspaceSettings, String> {
        match group {
            Some(id) => execution_settings(
                &self
                    .state()
                    .policy_store
                    .get(project, "group", id)
                    .await?
                    .body["workspace"],
            ),
            None => Ok(WorkspaceSettings::default()),
        }
    }
}

pub(super) fn validated_settings(
    state: &AppState,
    actor: &str,
    value: Option<&Value>,
) -> Result<Value, String> {
    let Some(value) = value.filter(|v| !v.is_null()) else {
        return Ok(Value::Null);
    };
    if actor != state.node_id {
        return Err("workspace settings can only be changed on the owning node".into());
    }
    let settings: WorkspaceSettings =
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    settings.validate()?;
    Ok(json!(settings))
}

pub(super) fn execution_settings(value: &Value) -> Result<WorkspaceSettings, String> {
    let settings: WorkspaceSettings = if value.is_null() {
        WorkspaceSettings::default()
    } else {
        serde_json::from_value(value.clone()).map_err(|e| e.to_string())?
    };
    settings.validate()?;
    Ok(settings)
}
