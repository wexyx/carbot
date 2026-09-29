use crate::configuration::Settings;
use agent_runtime::config::RuntimeConfig;
use serde_json::{Value, json};
use std::collections::BTreeMap;

pub(super) struct AgentConfiguration;
impl AgentConfiguration {
    pub(super) fn merge(previous: &Value, update: Option<&Value>) -> Result<Value, String> {
        let mut values: BTreeMap<String, String> = if previous.is_object() {
            serde_json::from_value(previous.clone()).map_err(|e| e.to_string())?
        } else {
            BTreeMap::new()
        };
        if let Some(update) = update {
            if update.is_null() {
                values.clear();
            } else {
                let updates: BTreeMap<String, String> = serde_json::from_value(update.clone())
                    .map_err(|_| "configuration must contain string values")?;
                if updates.contains_key("ADMIN_AGENT_PROVIDER")
                    || updates.iter().any(|(key, value)| {
                        value.len() > if key == "AGENT_ENV_JSON" { 65536 } else { 8192 }
                    })
                {
                    return Err("invalid Agent configuration".into());
                }
                values = Settings::merge_values(values, updates)?;
            }
        }
        Ok(json!(values))
    }
    pub(super) fn public(value: &Value) -> Value {
        let mut values = value.as_object().cloned().unwrap_or_default();
        values.remove("CONTEXT_STRATEGY");
        let has_key = values
            .remove("MODEL_API_KEY")
            .and_then(|v| v.as_str().map(|s| !s.is_empty()))
            .unwrap_or(false);
        if let Some(raw) = values.get("AGENT_ENV_JSON").and_then(Value::as_str) {
            let masked = agent_runtime::environment::AgentEnvironment::from_json(raw)
                .map(|env| env.masked_json())
                .unwrap_or_else(|_| "{}".into());
            values.insert("AGENT_ENV_JSON".into(), json!(masked));
        }
        json!({"values":values,"has_api_key":has_key,"inherits_instance":value.as_object().is_none_or(|v|v.is_empty())})
    }
    pub(super) fn runtime(
        provider: &str,
        definition: Option<&Value>,
    ) -> Result<RuntimeConfig, String> {
        let mut settings = Settings::load()?;
        if let Some(values) = definition
            .and_then(|v| v.get("configuration"))
            .filter(|v| v.is_object())
        {
            if values.as_object().is_some_and(|v| !v.is_empty())
                && values.get("AGENT_ENV_JSON").is_none()
            {
                // An independently configured Agent must not silently inherit another Agent's env secrets.
                settings.set("AGENT_ENV_JSON", "{}".into());
            }
            settings.update(serde_json::from_value(values.clone()).map_err(|e| e.to_string())?)?;
        }
        settings.set("ADMIN_AGENT_PROVIDER", provider.into());
        settings.runtime()
    }
    pub(super) fn redact(mut row: Value) -> Value {
        row["configuration"] = Self::public(&row["configuration"]);
        row
    }
}
