use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Project-owned, portable UTF-8 skill package; no executable is fetched implicitly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SkillDefinition {
    id: String,
    description: String,
    #[serde(default)]
    enabled: bool,
    #[serde(default)]
    allow_python: bool,
    #[serde(default)]
    execution_profile: Option<String>,
    files: BTreeMap<String, String>,
}
impl SkillDefinition {
    pub fn new(
        id: String,
        description: String,
        files: BTreeMap<String, String>,
        enabled: bool,
        allow_python: bool,
    ) -> Result<Self, String> {
        let skill = Self {
            id,
            description,
            files,
            enabled,
            allow_python,
            execution_profile: None,
        };
        skill.validate()?;
        Ok(skill)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.execution_profile.as_ref().is_some_and(|p| {
            p.is_empty()
                || p.len() > 64
                || p.starts_with('-')
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        }) {
            return Err("invalid skill execution profile".into());
        }
        if self.id.is_empty()
            || self.id.len() > 64
            || !self
                .id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
        {
            return Err("skill id must contain 1..64 ASCII letters, digits, - or _".into());
        }
        if self.description.is_empty() || self.description.len() > 2048 {
            return Err("skill description required (maximum 2 KiB)".into());
        }
        if !self
            .files
            .get("SKILL.md")
            .is_some_and(|v| !v.trim().is_empty())
        {
            return Err("nonempty SKILL.md required".into());
        }
        if self.files.len() > 32 || self.files.values().map(String::len).sum::<usize>() > 256 * 1024
        {
            return Err("skill limit: 32 files, 256 KiB total".into());
        }
        for (path, content) in &self.files {
            Self::validate_path(path)?;
            if content.len() > 64 * 1024 {
                return Err("skill file exceeds 64 KiB".into());
            }
        }
        Ok(())
    }
    pub(crate) fn validate_path(path: &str) -> Result<(), String> {
        if path.is_empty()
            || path.len() > 200
            || path
                .split('/')
                .any(|s| s.is_empty() || s == "." || s == ".." || s.starts_with('.'))
            || !path
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"/-_.".contains(&c))
        {
            return Err("skill paths must be safe relative paths without hidden components".into());
        }
        Ok(())
    }
    pub fn id(&self) -> &str {
        &self.id
    }
    pub fn enabled(&self) -> bool {
        self.enabled
    }
    pub fn description(&self) -> &str {
        &self.description
    }
    pub fn allow_python(&self) -> bool {
        self.allow_python
    }
    pub fn execution_profile(&self) -> Option<&str> {
        self.execution_profile.as_deref()
    }
    pub fn files(&self) -> &BTreeMap<String, String> {
        &self.files
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_packages() {
        for path in [
            "../secret",
            "/tmp/x",
            "scripts/../../x",
            "scripts\\x.py",
            ".env",
            "a//b",
        ] {
            assert!(SkillDefinition::validate_path(path).is_err());
        }
        assert!(
            SkillDefinition::new("ok".into(), "test".into(), BTreeMap::new(), true, false).is_err()
        );
        assert!(SkillDefinition::validate_path("scripts/main.py").is_ok());
    }
}
