use std::{
    io::{IsTerminal, Write},
    process::{Command, Stdio},
};
use tokio::io::{AsyncBufRead, Lines};

struct EchoGuard(String);
impl EchoGuard {
    fn hide() -> Result<Self, String> {
        let output = Command::new("stty")
            .arg("-g")
            .stdin(Stdio::inherit())
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("Cannot hide secret input on this terminal".into());
        }
        let state = String::from_utf8(output.stdout).map_err(|e| e.to_string())?;
        let status = Command::new("stty")
            .arg("-echo")
            .stdin(Stdio::inherit())
            .status()
            .map_err(|e| e.to_string())?;
        if !status.success() {
            return Err("Cannot hide secret input on this terminal".into());
        }
        Ok(Self(state.trim().into()))
    }
}
impl Drop for EchoGuard {
    fn drop(&mut self) {
        let _ = Command::new("stty")
            .arg(&self.0)
            .stdin(Stdio::inherit())
            .status();
        println!();
    }
}
pub(super) async fn ask<R: AsyncBufRead + Unpin>(
    lines: &mut Lines<R>,
    label: &str,
    default: &str,
    secret: bool,
) -> Result<String, String> {
    let shown = if secret && !default.is_empty() {
        "已设置，回车保留"
    } else if secret {
        "不回显"
    } else {
        default
    };
    print!("{label} [{shown}]: ");
    std::io::stdout().flush().map_err(|e| e.to_string())?;
    let _guard = if secret && std::io::stdin().is_terminal() {
        Some(EchoGuard::hide()?)
    } else {
        None
    };
    let input = tokio::select! {
        input = lines.next_line() => input,
        _ = tokio::signal::ctrl_c() => return Err("配置已取消；未保存更改".into()),
    }
    .map_err(|e| e.to_string())?
    .ok_or("配置已取消（输入结束）；未保存更改")?;
    if input.trim() == "/cancel" {
        return Err("配置已取消；未保存更改".into());
    }
    if input.trim() == "-" {
        return Ok(String::new());
    }
    Ok(if input.trim().is_empty() {
        default.into()
    } else {
        input.trim().into()
    })
}
