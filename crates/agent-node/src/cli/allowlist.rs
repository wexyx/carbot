use crate::core::Core;
use serde_json::json;

pub(super) async fn execute(core: &Core, args: &str) -> Result<String, String> {
    let current = core.command_allowlist().await;
    let rest = args;
    let mut commands: Vec<String> =
        serde_json::from_value(current["command_allowlist"].clone()).map_err(|e| e.to_string())?;
    let rest = rest.trim();
    if !rest.is_empty() {
        let (action, command) = rest.split_once(char::is_whitespace).unwrap_or((rest, ""));
        match action {
            "add" if !command.trim().is_empty() => commands.push(command.trim().into()),
            "remove" if !command.trim().is_empty() => commands.retain(|s| s != command.trim()),
            "clear" if command.is_empty() => commands.clear(),
            "reset" if command.is_empty() => {
                commands = serde_json::from_value(current["default_allowlist"].clone())
                    .map_err(|e| e.to_string())?
            }
            _ => {
                return Err("用法：/allowlist [add COMMAND|remove COMMAND|reset|clear]".into());
            }
        }
        core.set_command_allowlist(
            json!({"expected_version":current["version"],"command_allowlist":commands}),
        )
        .await?;
    }
    Ok(format!(
        "{} · 自动批准白名单（{} 条）\n{}\n仅「帮我批准」生效，完整命令支持 * / ? 通配符，拒绝 shell 复合语句；所有本地 Agent 的后续项目任务生效。",
        "全局",
        commands.len(),
        commands.join("\n")
    ))
}
