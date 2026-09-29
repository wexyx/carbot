pub(super) enum Command {
    Help(String),
    Workbench(String, serde_json::Value),
    History(String),
    Tools(Option<usize>),
    Exit,
    Project(Option<String>),
    New,
    Resume(String),
    Chat(String),
    Admin,
    AdminConfig,
    Permissions(String),
    Allowlist(String),
    Interrupt,
    Confirm(String, bool),
    WorkspaceConfirm(String, bool),
    Say(String),
    Group(String),
}
pub(super) fn parse(line: &str) -> Result<Command, String> {
    let line = line.trim();
    if !line.starts_with('/') {
        return Ok(Command::Say(line.into()));
    }
    let (head, tail) = line[1..].split_once(' ').unwrap_or((&line[1..], ""));
    if let Some(command) = super::workbench_commands::parse(head, tail)? {
        return Ok(command);
    }
    Ok(match head {
        "add-agent" | "remove-agent" | "agent" | "group" => Command::Group(line.into()),
        "members" => Command::Group("/agents".into()),
        "manage" => Command::Admin,
        "history" => Command::History(tail.trim().into()),
        "tools" => Command::Tools(if tail.trim().is_empty() {
            None
        } else {
            Some(tail.trim().parse().map_err(|_| "usage: /tools [number]")?)
        }),
        "help" => Command::Help(tail.trim().into()),
        "exit" => Command::Exit,
        "namespace" => Command::Project((!tail.trim().is_empty()).then(|| tail.trim().into())),
        "allowlist" => Command::Allowlist(tail.trim().into()),
        "permissions" => Command::Permissions(tail.trim().into()),
        "new" => Command::New,
        "resume" => Command::Resume(tail.trim().into()),
        "chat" => Command::Chat(tail.trim().into()),
        "admin" => Command::Admin,
        "agent-config" | "admin-config" => Command::AdminConfig,
        "interrupt" => Command::Interrupt,
        "approve" | "deny" => Command::Confirm(tail.trim().into(), head == "approve"),
        "allow-path" | "deny-path" => {
            Command::WorkspaceConfirm(tail.trim().into(), head == "allow-path")
        }
        _ => return Err("仅支持会话导航与人工确认命令；管理操作可以直接告诉默认 Agent。".into()),
    })
}
pub(super) use super::help::TEXT as HELP;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn management_is_natural_language_not_a_command() {
        assert!(matches!(parse("web start").unwrap(), Command::Say(_)));
        assert!(matches!(parse("帮我建群").unwrap(), Command::Say(_)));
        assert!(parse("/tool agent_start {}").is_err());
        assert!(matches!(
            parse("/approve abc").unwrap(),
            Command::Confirm(_, true)
        ));
    }
}
