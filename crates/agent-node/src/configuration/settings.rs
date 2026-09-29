use agent_runtime::config::{ClaudeConfig, CodexConfig, HarnessConfig, RuntimeConfig};
use std::{collections::BTreeMap, io::Write, path::PathBuf};

const KEYS: &[&str] = &[
    "ADMIN_AGENT_PROVIDER",
    "MODEL_PROVIDER",
    "MODEL_NAME",
    "MODEL_API_KEY",
    "MODEL_BASE_URL",
    "MODEL_API",
    "CONTEXT_MAX_TOKENS",
    "CONTEXT_STRATEGY",
    "CODEX_BIN",
    "CLAUDE_BIN",
];

#[derive(Default, Clone)]
pub(crate) struct Settings {
    values: BTreeMap<String, String>,
}
impl Settings {
    fn path() -> PathBuf {
        PathBuf::from(std::env::var_os("CARBOT_DATA_DIR").unwrap_or_else(|| ".carbot".into()))
            .join("admin-agent.json")
    }
    pub(crate) fn load() -> Result<Self, String> {
        let mut settings = match std::fs::read(Self::path()) {
            Ok(bytes) => Self {
                values: serde_json::from_slice(&bytes).map_err(
                    |_| "Invalid admin-agent.json; repair or move that file before configuring",
                )?,
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(e) => return Err(e.to_string()),
        };
        for key in KEYS {
            if let Ok(value) = std::env::var(key) {
                // Launcher dotenv values are defaults, not explicit operator overrides.
                // A saved interactive configuration must survive stale/blank template files.
                let defaults = std::env::var("CARBOT_CONFIG_DEFAULT_KEYS").unwrap_or_default();
                if defaults.split_whitespace().any(|item| item == *key)
                    && settings.values.contains_key(*key)
                {
                    continue;
                }
                settings.set(key, value);
            }
        }
        Ok(settings)
    }
    pub(crate) fn get(&self, key: &str) -> &str {
        self.values.get(key).map(String::as_str).unwrap_or("")
    }
    pub(crate) fn public_view(&self) -> serde_json::Value {
        let mut values = self.values.clone();
        values.remove("MODEL_API_KEY");
        serde_json::json!({"values": values, "has_api_key": !self.get("MODEL_API_KEY").is_empty()})
    }
    pub(crate) fn update(&mut self, values: BTreeMap<String, String>) -> Result<(), String> {
        if values.keys().any(|key| !KEYS.contains(&key.as_str())) {
            return Err("unknown AdminAgent configuration field".into());
        }
        agent_runtime::context::ContextBudget::from_lookup(|key| {
            values
                .get(key)
                .cloned()
                .or_else(|| self.values.get(key).cloned())
        })?;
        self.values.extend(values);
        Ok(())
    }
    pub(crate) fn set(&mut self, key: &str, value: String) {
        self.values.insert(key.into(), value);
    }
    pub(crate) fn runtime(&self) -> Result<RuntimeConfig, String> {
        match self.get("ADMIN_AGENT_PROVIDER") {
            "" | "carbot" => Ok(RuntimeConfig::Carbot(HarnessConfig::from_lookup(|key| {
                self.values
                    .get(key)
                    .cloned()
                    .or_else(|| std::env::var(key).ok())
            })?)),
            "codex" => {
                let mut cfg = CodexConfig::from_env();
                if !self.get("CODEX_BIN").is_empty() {
                    cfg.binary = self.get("CODEX_BIN").into();
                }
                Ok(RuntimeConfig::Codex(cfg))
            }
            "claude" => {
                let mut cfg = ClaudeConfig::from_env();
                if !self.get("CLAUDE_BIN").is_empty() {
                    cfg.binary = self.get("CLAUDE_BIN").into();
                }
                Ok(RuntimeConfig::Claude(cfg))
            }
            "mock" => Ok(RuntimeConfig::Mock),
            _ => Err("ADMIN_AGENT_PROVIDER must be carbot, codex, claude or mock".into()),
        }
    }
    pub(crate) fn save(&self) -> Result<(), String> {
        self.save_to(&Self::path())
    }
    fn save_to(&self, path: &std::path::Path) -> Result<(), String> {
        let parent = path
            .parent()
            .ok_or("configuration requires a parent directory")?;
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        // tempfile creates an owner-only file on Unix; rename avoids partial configuration writes.
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut file, &self.values).map_err(|e| e.to_string())?;
        file.flush().map_err(|e| e.to_string())?;
        file.as_file().sync_all().map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.error.to_string())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_saves_private_literal_configuration() {
        let dir = tempfile::tempdir().unwrap();
        let mut cfg = Settings::default();
        cfg.set("MODEL_PROVIDER", "invalid".into());
        assert!(cfg.runtime().is_err());
        cfg.set("MODEL_PROVIDER", "ollama".into());
        cfg.set("MODEL_NAME", "fixture".into());
        assert!(cfg.runtime().is_ok());
        cfg.set("MODEL_API_KEY", "literal $() secret".into());
        assert!(!cfg.public_view().to_string().contains("literal $() secret"));
        assert!(
            cfg.update(BTreeMap::from([("AGENT_WORKDIR".into(), "/".into())]))
                .is_err()
        );
        let path = dir.path().join("admin-agent.json");
        cfg.save_to(&path).unwrap();
        let saved: BTreeMap<String, String> =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved["MODEL_API_KEY"], "literal $() secret");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        for provider in ["codex", "claude", "mock"] {
            cfg.set("ADMIN_AGENT_PROVIDER", provider.into());
            assert!(cfg.runtime().is_ok());
        }
    }
}
