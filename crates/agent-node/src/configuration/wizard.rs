use super::Settings;

pub(crate) struct Wizard {
    settings: Settings,
    step: usize,
}
pub(crate) struct Field {
    pub(crate) label: &'static str,
    pub(crate) default: String,
    pub(crate) secret: bool,
    key: &'static str,
}
impl Wizard {
    pub(crate) fn new(settings: Settings) -> Self {
        Self { settings, step: 0 }
    }
    pub(crate) fn field(&self) -> Field {
        let (key, label, default, secret) = if self.step == 0 {
            (
                "ADMIN_AGENT_PROVIDER",
                "运行器 carbot / codex / claude",
                "carbot",
                false,
            )
        } else if self.settings.get("ADMIN_AGENT_PROVIDER") == "codex" {
            (
                "CODEX_BIN",
                "Codex 可执行文件（需安装和认证）",
                "codex",
                false,
            )
        } else if self.settings.get("ADMIN_AGENT_PROVIDER") == "claude" {
            (
                "CLAUDE_BIN",
                "Claude 可执行文件（需安装和认证）",
                "claude",
                false,
            )
        } else {
            [
                (
                    "MODEL_PROVIDER",
                    "厂商 openai / anthropic / gemini / deepseek / qwen / ark / ollama / compatible",
                    "openai",
                    false,
                ),
                ("MODEL_NAME", "模型名称（支持工具调用）", "", false),
                ("MODEL_BASE_URL", "接口地址（空值使用厂商默认）", "", false),
                (
                    "MODEL_API",
                    "协议 chat / responses / anthropic（空值使用厂商默认）",
                    "",
                    false,
                ),
                (
                    "MODEL_API_KEY",
                    "API Key（ollama 可空；本地明文 0600 保存）",
                    "",
                    true,
                ),
            ][self.step - 1]
        };
        let current = self.settings.get(key);
        Field {
            key,
            label,
            default: if current.is_empty() {
                default.into()
            } else {
                current.into()
            },
            secret,
        }
    }
    pub(crate) fn prompt(&self) -> String {
        let field = self.field();
        let default = if field.secret {
            if field.default.is_empty() {
                "未设置"
            } else {
                "已设置，回车保留"
            }
        } else {
            &field.default
        };
        format!(
            "配置 {}/{} · {} [{}] · - 清空 / Esc 取消",
            self.step + 1,
            if self.step > 0 && self.settings.get("ADMIN_AGENT_PROVIDER") != "carbot" {
                2
            } else {
                6
            },
            field.label,
            default
        )
    }
    pub(crate) fn submit(&mut self, raw: String) -> Result<Option<Settings>, String> {
        let value = if raw.trim().is_empty() {
            self.field().default
        } else if raw.trim() == "-" {
            String::new()
        } else {
            raw.trim().into()
        };
        self.accept(value)
    }
    pub(crate) fn accept(&mut self, value: String) -> Result<Option<Settings>, String> {
        if self.step == 0 && !matches!(value.as_str(), "carbot" | "codex" | "claude") {
            return Err("请选择 carbot、codex 或 claude。".into());
        }
        self.settings.set(self.field().key, value);
        self.step += 1;
        let end = if self.settings.get("ADMIN_AGENT_PROVIDER") == "carbot" {
            6
        } else {
            2
        };
        if self.step >= end {
            self.step = 0;
            self.settings.runtime()?;
            return Ok(Some(self.settings.clone()));
        }
        Ok(None)
    }
}
