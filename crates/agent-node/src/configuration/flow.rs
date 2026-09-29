use super::{Settings, terminal::ask};
use agent_runtime::config::RuntimeConfig;
use std::io::IsTerminal;
use tokio::io::{AsyncBufRead, AsyncBufReadExt, Lines};

pub(crate) async fn startup() -> Result<RuntimeConfig, String> {
    let settings = Settings::load()?;
    match settings.runtime() {
        Ok(config) => Ok(config),
        Err(error) => {
            if !std::io::stdin().is_terminal() {
                return Err(format!(
                    "AdminAgent configuration: {error}. No interactive terminal; run ./carbot in a terminal to configure, or set ADMIN_AGENT_PROVIDER / MODEL_* in the environment."
                ));
            }
            println!("AdminAgent 配置未完成：{error}。进入配置向导。");
            let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
            let settings = edit(settings, &mut lines).await?;
            settings.save()?;
            println!("AdminAgent 配置已保存，继续启动。");
            settings.runtime()
        }
    }
}

pub(crate) async fn edit<R: AsyncBufRead + Unpin>(
    settings: Settings,
    lines: &mut Lines<R>,
) -> Result<Settings, String> {
    println!(
        "AdminAgent 配置：回车保留默认值，- 清空，/cancel 取消。配置保存在 CARBOT_DATA_DIR/admin-agent.json（API Key 为明文，文件权限 0600）。"
    );
    let mut wizard = super::Wizard::new(settings);
    loop {
        let field = wizard.field();
        let value = ask(lines, field.label, &field.default, field.secret).await?;
        match wizard.accept(value) {
            Ok(Some(settings)) => return Ok(settings),
            Ok(None) => {}
            Err(e) => println!("配置无效：{e}，请重新填写。"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn retries_invalid_provider_then_configures_cli_without_model_key() {
        let mut lines = tokio::io::BufReader::new(&b"wrong\ncodex\n/test/codex\n"[..]).lines();
        let cfg = edit(Settings::default(), &mut lines).await.unwrap();
        assert!(matches!(cfg.runtime().unwrap(), RuntimeConfig::Codex(_)));
        assert_eq!(cfg.get("CODEX_BIN"), "/test/codex");
    }
    #[tokio::test]
    async fn eof_and_cancel_do_not_loop() {
        for text in ["", "/cancel\n"] {
            let mut lines = tokio::io::BufReader::new(text.as_bytes()).lines();
            assert!(edit(Settings::default(), &mut lines).await.is_err());
        }
    }
}
