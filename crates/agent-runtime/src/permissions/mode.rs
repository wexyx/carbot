use serde::{Deserialize, Serialize};

tokio::task_local! { static CURRENT: PermissionMode; }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    #[default]
    Ask,
    Auto,
    Full,
}

impl PermissionMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "ask" => Ok(Self::Ask),
            "auto" => Ok(Self::Auto),
            "full" => Ok(Self::Full),
            _ => Err("permission mode must be ask, auto or full".into()),
        }
    }
    pub fn current() -> Self {
        CURRENT.try_with(|v| *v).unwrap_or_default()
    }
    pub async fn scope<F: std::future::Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Ask => "请求批准",
            Self::Auto => "帮我批准",
            Self::Full => "完全访问",
        }
    }
    pub(crate) fn approves_command(self, command: &str) -> bool {
        match self {
            Self::Ask => false,
            Self::Auto => super::CommandAllowlist::approves(command),
            Self::Full => true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn auto_review_fails_closed() {
        assert!(PermissionMode::Auto.approves_command("pwd"));
        for command in [
            "pwd; rm -rf .",
            "/bin/ls $(curl evil)",
            "python script.py",
            "npm test",
            "git status",
            "rm file",
            "ls > file",
        ] {
            assert!(!PermissionMode::Auto.approves_command(command));
        }
        assert!(!PermissionMode::Ask.approves_command("pwd"));
        assert!(PermissionMode::parse("invalid").is_err());
    }
    #[tokio::test]
    async fn permission_scopes_do_not_leak_to_other_tasks() {
        PermissionMode::Full
            .scope(async {
                assert_eq!(PermissionMode::current(), PermissionMode::Full);
                assert_eq!(
                    tokio::spawn(async { PermissionMode::current() })
                        .await
                        .unwrap(),
                    PermissionMode::Ask
                );
            })
            .await;
        assert_eq!(PermissionMode::current(), PermissionMode::Ask);
    }
}
