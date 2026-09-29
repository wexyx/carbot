use std::future::Future;

tokio::task_local! { static CURRENT: CommandAllowlist; }

/// Human-approved complete command patterns; shell composition is always rejected.
#[derive(Clone, Debug)]
pub struct CommandAllowlist {
    commands: Vec<String>,
}

impl Default for CommandAllowlist {
    fn default() -> Self {
        let mut commands = vec![
            "pwd",
            "/bin/pwd",
            "ls",
            "ls -l",
            "ls -la",
            "ls -a",
            "ls -lh",
            "/bin/ls",
            "/bin/ls -l",
            "/bin/ls -la",
            "/bin/ls -a",
            "/bin/ls -lh",
            "whoami",
            "id",
            "id -u",
            "id -g",
            "uname",
            "uname -a",
            "uname -s",
            "uname -m",
            "hostname",
            "date",
            "date -u",
            "uptime",
            "getconf _NPROCESSORS_ONLN",
            "sw_vers",
            "sw_vers -productVersion",
            "git --version",
            "rg --version",
            "rg --files",
            "rg --files --hidden",
            "find . -maxdepth 1 -type d",
            "find . -maxdepth 2 -type d",
            "wc -l README.md",
            "cat README.md",
            "cat Cargo.toml",
            "cat package.json",
            "head -n 40 README.md",
            "tail -n 40 README.md",
        ]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
        // Disable configured fsmonitor execution; no external diff/textconv or pager.
        for args in [
            "status --short",
            "status --porcelain",
            "status --branch --short",
            "ls-files",
            "branch --show-current",
            "rev-parse --show-toplevel",
            "log -5 --oneline",
            "diff --no-ext-diff --no-textconv --stat",
            "diff --no-ext-diff --no-textconv",
            "diff --cached --no-ext-diff --no-textconv --stat",
        ] {
            commands.push(format!("git --no-pager -c core.fsmonitor=false {args}"));
        }
        Self { commands }
    }
}

impl CommandAllowlist {
    pub fn new(commands: Vec<String>) -> Result<Self, String> {
        if commands.len() > 256 {
            return Err("白名单最多 256 条命令".into());
        }
        let mut result = Vec::new();
        for command in commands {
            let command = command.trim();
            if command.len() > 2048 || !super::command_pattern::valid_pattern(command) {
                return Err("白名单支持 * 和 ? 通配符；可执行文件名必须明确，不支持管道、重定向、命令替换或复合 shell 语句".into());
            }
            if !result.iter().any(|v| v == command) {
                result.push(command.to_owned());
            }
        }
        Ok(Self { commands: result })
    }
    pub fn commands(&self) -> &[String] {
        &self.commands
    }
    pub(crate) fn approves(command: &str) -> bool {
        CURRENT
            .try_with(|list| {
                list.commands
                    .iter()
                    .any(|v| super::command_pattern::matches(v, command))
            })
            .unwrap_or_else(|_| {
                Self::default()
                    .commands
                    .iter()
                    .any(|v| super::command_pattern::matches(v, command))
            })
    }
    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        CURRENT.scope(self, future).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn defaults_are_valid_and_compound_commands_are_rejected() {
        let defaults = CommandAllowlist::default();
        assert!(defaults.commands().len() >= 40);
        assert!(CommandAllowlist::new(defaults.commands().to_vec()).is_ok());
        for command in [
            "pwd; rm file",
            "ls > file",
            "ls $(id)",
            "pwd\nid",
            "ls | sh",
        ] {
            assert!(CommandAllowlist::new(vec![command.into()]).is_err());
        }
    }
    #[tokio::test]
    async fn exact_custom_entries_and_empty_override() {
        CommandAllowlist::new(vec!["cat notes.txt".into()])
            .unwrap()
            .scope(async {
                assert!(CommandAllowlist::approves("cat notes.txt"));
                assert!(!CommandAllowlist::approves("cat notes.txt other.txt"));
                assert!(!CommandAllowlist::approves("pwd"));
            })
            .await;
        CommandAllowlist::new(vec![])
            .unwrap()
            .scope(async {
                assert!(!CommandAllowlist::approves("pwd"));
            })
            .await;
        assert!(CommandAllowlist::approves("pwd"));
    }
}
