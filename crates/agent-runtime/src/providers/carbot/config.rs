use std::{env, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelApi {
    Chat,
    Responses,
    Anthropic,
}

#[derive(Clone)]
pub struct HarnessConfig {
    pub context: crate::context::ContextBudget,
    pub api: ModelApi,
    pub base: String,
    pub key: String,
    pub model: String,
    pub max_steps: usize,
    pub max_tokens: u64,
    pub root: PathBuf,
}

impl HarnessConfig {
    pub(crate) fn validate(mut self) -> Result<Self, String> {
        if !(self.base.starts_with("http://") || self.base.starts_with("https://")) {
            return Err("MODEL_BASE_URL must be an HTTP(S) URL".into());
        }
        if self.model.is_empty() {
            return Err("MODEL_NAME is required; select a model with tool-calling support".into());
        }
        if !(1..=64).contains(&self.max_steps) {
            return Err("HARNESS_MAX_STEPS must be 1..64".into());
        }
        self.context.input_limit(self.max_tokens)?;
        self.root = std::fs::canonicalize(self.root).map_err(|_| "AGENT_WORKDIR does not exist")?;
        self.base = self.base.trim_end_matches('/').into();
        Ok(self)
    }
    pub fn from_env() -> Result<Self, String> {
        Self::from_lookup(|key| env::var(key).ok())
    }

    pub fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let vendor = get("MODEL_PROVIDER").unwrap_or_else(|| "openai".into());
        let (api, base) = match vendor.as_str() {
            "openai" => (ModelApi::Responses, "https://api.openai.com/v1"),
            "anthropic" => (ModelApi::Anthropic, "https://api.anthropic.com/v1"),
            "gemini" => (
                ModelApi::Chat,
                "https://generativelanguage.googleapis.com/v1beta/openai",
            ),
            "deepseek" => (ModelApi::Chat, "https://api.deepseek.com/v1"),
            "qwen" => (
                ModelApi::Chat,
                "https://dashscope.aliyuncs.com/compatible-mode/v1",
            ),
            "ark" => (ModelApi::Chat, "https://ark.cn-beijing.volces.com/api/v3"),
            "ollama" => (ModelApi::Chat, "http://localhost:11434/v1"),
            "compatible" => (ModelApi::Chat, ""),
            _ => return Err("unknown MODEL_PROVIDER".into()),
        };
        let api = match get("MODEL_API").filter(|v| !v.is_empty()).as_deref() {
            None => api,
            Some("chat") => ModelApi::Chat,
            Some("responses") => ModelApi::Responses,
            Some("anthropic") => ModelApi::Anthropic,
            Some(_) => return Err("MODEL_API must be chat, responses, or anthropic".into()),
        };
        let base = get("MODEL_BASE_URL")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| base.into());
        if !(base.starts_with("http://") || base.starts_with("https://")) {
            return Err("MODEL_BASE_URL must be an HTTP(S) URL".into());
        }
        let model = get("MODEL_NAME")
            .filter(|m| !m.is_empty())
            .ok_or("MODEL_NAME is required; select a model with tool-calling support")?;
        let key = get("MODEL_API_KEY").unwrap_or_default();
        if key.is_empty() && vendor != "ollama" {
            return Err("MODEL_API_KEY is required".into());
        }
        let context = crate::context::ContextBudget::from_lookup(&get)?;
        let max_steps = get("HARNESS_MAX_STEPS")
            .unwrap_or_else(|| "12".into())
            .parse()
            .map_err(|_| "invalid HARNESS_MAX_STEPS")?;
        if !(1..=64).contains(&max_steps) {
            return Err("HARNESS_MAX_STEPS must be 1..64".into());
        }
        let max_tokens = get("HARNESS_MAX_TOKENS")
            .unwrap_or_else(|| "4096".into())
            .parse()
            .map_err(|_| "invalid HARNESS_MAX_TOKENS")?;
        let root = std::fs::canonicalize(get("AGENT_WORKDIR").unwrap_or_else(|| ".".into()))
            .map_err(|_| "AGENT_WORKDIR does not exist")?;
        context.input_limit(max_tokens)?;
        Ok(Self {
            context,
            api,
            base: base.trim_end_matches('/').into(),
            key,
            model,
            max_steps,
            max_tokens,
            root,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn config(vendor: &str, extra: &[(&str, &str)]) -> Result<HarnessConfig, String> {
        HarnessConfig::from_lookup(|key| {
            if let Some((_, v)) = extra.iter().find(|(k, _)| *k == key) {
                return Some((*v).into());
            }
            match key {
                "MODEL_PROVIDER" => Some(vendor.into()),
                "MODEL_NAME" => Some("fixture".into()),
                "MODEL_API_KEY" => Some("fixture-key".into()),
                "AGENT_WORKDIR" => Some(env!("CARGO_MANIFEST_DIR").into()),
                _ => None,
            }
        })
    }
    #[test]
    fn vendor_defaults_and_protocol_overrides_remain_compatible() {
        for (vendor, api) in [
            ("openai", ModelApi::Responses),
            ("anthropic", ModelApi::Anthropic),
            ("gemini", ModelApi::Chat),
            ("deepseek", ModelApi::Chat),
            ("qwen", ModelApi::Chat),
            ("ark", ModelApi::Chat),
            ("ollama", ModelApi::Chat),
        ] {
            let cfg = config(vendor, &[]).unwrap();
            assert_eq!(cfg.api, api);
            assert!(cfg.base.starts_with("http"));
        }
        let cfg = config(
            "compatible",
            &[
                ("MODEL_BASE_URL", "http://localhost:1234/v1/"),
                ("MODEL_API", "responses"),
            ],
        )
        .unwrap();
        assert_eq!(cfg.api, ModelApi::Responses);
        assert_eq!(cfg.base, "http://localhost:1234/v1");
    }
    #[test]
    fn config_validation_does_not_mutate_process_environment() {
        assert!(config("compatible", &[]).is_err());
        assert!(config("unknown", &[]).is_err());
        assert!(config("openai", &[("MODEL_API", "invalid")]).is_err());
        assert!(config("openai", &[("MODEL_API_KEY", "")]).is_err());
        assert!(config("ollama", &[("MODEL_API_KEY", "")]).is_ok());
        assert!(config("openai", &[("HARNESS_MAX_STEPS", "0")]).is_err());
    }
}
