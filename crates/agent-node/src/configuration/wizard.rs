use super::Settings;
use agent_runtime::config::OpenCodeModel;

/// One prompt in a runner's configuration plan.
struct Entry {
    key: &'static str,
    label: &'static str,
    default: &'static str,
    secret: bool,
}

/// A prompt with the stored value already resolved as its default.
pub(crate) struct Field {
    pub(crate) label: &'static str,
    pub(crate) default: String,
    pub(crate) secret: bool,
    pub(crate) key: &'static str,
}

const RUNNER: Entry = Entry {
    key: "ADMIN_AGENT_PROVIDER",
    label: "运行器 carbot / codex / claude / opencode",
    default: "carbot",
    secret: false,
};
const MODEL_PROVIDER: Entry = Entry {
    key: "MODEL_PROVIDER",
    label: "厂商 openai / anthropic / gemini / deepseek / qwen / ark / ollama / compatible",
    default: "openai",
    secret: false,
};
const MODEL_NAME: Entry = Entry {
    key: "MODEL_NAME",
    label: "模型名称（支持工具调用）",
    default: "",
    secret: false,
};
const MODEL_BASE_URL: Entry = Entry {
    key: "MODEL_BASE_URL",
    label: "接口地址（空值使用厂商默认）",
    default: "",
    secret: false,
};
const MODEL_API: Entry = Entry {
    key: "MODEL_API",
    label: "协议 chat / responses / anthropic（空值使用厂商默认）",
    default: "",
    secret: false,
};
const MODEL_API_KEY: Entry = Entry {
    key: "MODEL_API_KEY",
    label: "API Key（ollama 可空；本地明文 0600 保存）",
    default: "",
    secret: true,
};
const CODEX_BIN: Entry = Entry {
    key: "CODEX_BIN",
    label: "Codex 启动命令（可带参数，需安装和认证）",
    default: "codex",
    secret: false,
};
const CLAUDE_BIN: Entry = Entry {
    key: "CLAUDE_BIN",
    label: "Claude 启动命令（可带参数，需安装和认证）",
    default: "claude",
    secret: false,
};
const OPENCODE_BIN: Entry = Entry {
    key: "OPENCODE_BIN",
    label: "OpenCode 启动命令（可带参数，需安装和认证）",
    default: "opencode",
    secret: false,
};
const OPENCODE_MODEL: Entry = Entry {
    key: "OPENCODE_MODEL",
    label: "OpenCode 模型（填序号或 provider/model；留空使用默认模型）",
    default: "",
    secret: false,
};
const ENVIRONMENT: Entry = Entry {
    key: "AGENT_ENV_JSON",
    label: "环境变量 JSON（可选，如 {\"ANTHROPIC_API_KEY\":\"...\"}）",
    default: "",
    secret: true,
};

/// The runner decides the plan. OpenCode routes to models this node does not
/// host, so it asks for a model while Codex and Claude only ask for a launcher.
const CARBOT: &[Entry] = &[
    RUNNER,
    MODEL_PROVIDER,
    MODEL_NAME,
    MODEL_BASE_URL,
    MODEL_API,
    MODEL_API_KEY,
    ENVIRONMENT,
];
const CODEX: &[Entry] = &[RUNNER, CODEX_BIN, ENVIRONMENT];
const CLAUDE: &[Entry] = &[RUNNER, CLAUDE_BIN, ENVIRONMENT];
const OPENCODE: &[Entry] = &[RUNNER, OPENCODE_BIN, OPENCODE_MODEL, ENVIRONMENT];
/// Mock validates the transport only, so it configures nothing else.
const NONE: &[Entry] = &[RUNNER];

pub(crate) struct Wizard {
    settings: Settings,
    step: usize,
    /// Read once per wizard: the catalog only changes with the runner choice.
    catalog: Option<Result<Vec<OpenCodeModel>, String>>,
}
impl Wizard {
    pub(crate) fn new(settings: Settings) -> Self {
        Self {
            settings,
            step: 0,
            catalog: None,
        }
    }
    fn plan(&self) -> &'static [Entry] {
        let runner = self.settings.get("ADMIN_AGENT_PROVIDER");
        match if runner.is_empty() { "carbot" } else { runner } {
            "carbot" => CARBOT,
            "codex" => CODEX,
            "claude" => CLAUDE,
            "opencode" => OPENCODE,
            _ => NONE,
        }
    }
    /// The launcher as it stands, including a step answered but not yet committed.
    fn pending_launcher(&self) -> String {
        let launcher = self
            .plan()
            .iter()
            .position(|entry| entry.key == OPENCODE_BIN.key)
            .unwrap_or(usize::MAX);
        if self.step > launcher {
            self.settings.get(OPENCODE_BIN.key).into()
        } else {
            OPENCODE_BIN.default.into()
        }
    }
    /// Read the OpenCode catalog once, so every later step can reuse it.
    pub(crate) async fn refresh_catalog(&mut self) {
        if self.settings.get("ADMIN_AGENT_PROVIDER") != "opencode" || self.catalog.is_some() {
            return;
        }
        // The launcher being typed may not be saved yet, so use the in-progress value.
        let mut staged = self.settings.clone();
        staged.set("OPENCODE_BIN", self.pending_launcher());
        self.catalog = Some(match staged.opencode(None) {
            Ok(config) => agent_runtime::config::opencode_models(&config)
                .await
                .map(|catalog| catalog.models),
            Err(error) => Err(error),
        });
    }
    /// A picker number is only honoured on the model step, where the numbered
    /// list was actually shown.
    fn chosen_model(&self, field: &Field, raw: &str) -> Option<String> {
        if field.key != OPENCODE_MODEL.key {
            return None;
        }
        let index: usize = raw.trim().parse().ok()?;
        self.catalog
            .as_ref()?
            .as_ref()
            .ok()?
            .get(index.checked_sub(1)?)
            .map(|model| model.id.clone())
    }
    /// The numbered model list for the current step, or why it cannot be shown.
    pub(crate) fn catalog_text(&self) -> String {
        if self.field().key != OPENCODE_MODEL.key {
            return String::new();
        }
        match &self.catalog {
            None => "未能读取 OpenCode 模型目录，请确认已安装 opencode。\n".into(),
            Some(Err(error)) => format!("未能读取 OpenCode 模型目录：{error}\n"),
            Some(Ok(models)) if models.is_empty() => {
                "OpenCode 未登录，暂无可用模型；运行 opencode auth login 后重新配置，或直接填写模型 ID。\n".into()
            }
            Some(Ok(models)) => {
                let mut text = String::from("可用模型（免费优先）：\n");
                for (index, model) in models.iter().enumerate() {
                    text.push_str(&format!("  {:>2}) {}\n", index + 1, describe(model)));
                }
                text.push_str("填写序号选择，或直接输入 provider/model。\n");
                text
            }
        }
    }
    pub(crate) fn field(&self) -> Field {
        let entry = &self.plan()[self.step.min(self.plan().len() - 1)];
        let current = self.settings.get(entry.key);
        Field {
            key: entry.key,
            label: entry.label,
            default: if current.is_empty() {
                entry.default.into()
            } else {
                current.into()
            },
            secret: entry.secret,
        }
    }
    pub(crate) fn prompt(&self) -> String {
        let field = self.field();
        let default = if field.secret {
            if field.default.is_empty() {
                "未设置".into()
            } else {
                "已设置，回车保留".into()
            }
        } else {
            field.default.clone()
        };
        format!(
            "配置 {}/{} · {} [{}] · - 清空 / Esc 取消",
            self.step + 1,
            self.plan().len(),
            field.label,
            default
        )
    }
    pub(crate) async fn submit(&mut self, raw: String) -> Result<Option<Settings>, String> {
        let value = if raw.trim().is_empty() {
            self.field().default
        } else if raw.trim() == "-" {
            String::new()
        } else {
            raw.trim().into()
        };
        self.accept(value).await
    }
    pub(crate) async fn accept(&mut self, value: String) -> Result<Option<Settings>, String> {
        let field = self.field();
        if self.step == 0 && !matches!(value.as_str(), "carbot" | "codex" | "claude" | "opencode") {
            return Err("请选择 carbot、codex、claude 或 opencode。".into());
        }
        let value = self.chosen_model(&field, &value).unwrap_or(value);
        if field.key == ENVIRONMENT.key {
            self.settings.update(std::collections::BTreeMap::from([(
                ENVIRONMENT.key.into(),
                value,
            )]))?;
        } else {
            self.settings.set(field.key, value);
        }
        self.step += 1;
        // The runner may have just changed, so the plan and its catalog change with it.
        self.refresh_catalog().await;
        if self.step >= self.plan().len() {
            self.step = 0;
            self.settings.runtime()?;
            return Ok(Some(self.settings.clone()));
        }
        Ok(None)
    }
}
/// Free means every published tier costs zero; an unknown price is never "free".
fn describe(model: &OpenCodeModel) -> String {
    let price = if model.free {
        "免费".to_string()
    } else if model.priced {
        format!(
            "${:.2}/${:.2} 每百万 token",
            model.input.unwrap_or_default(),
            model.output.unwrap_or_default()
        )
    } else {
        "价格未知（仅能列出模型 ID）".into()
    };
    let context = match model.context {
        Some(context) => format!(" · {}K 上下文", context / 1000),
        None => String::new(),
    };
    let tools = if model.tools {
        ""
    } else {
        " · 不支持工具调用"
    };
    format!("{}{context}{tools} · {price}", model.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn free_is_spelled_out_and_an_unknown_price_is_not_free() {
        let model = |free, priced, input| OpenCodeModel {
            id: "opencode/demo".into(),
            name: "Demo".into(),
            free,
            input,
            output: input,
            context: Some(200_000),
            tools: true,
            status: "active".into(),
            priced,
        };
        assert!(describe(&model(true, true, Some(0.0))).contains("免费"));
        assert!(describe(&model(false, true, Some(2.0))).contains("$2.00/$2.00"));
        assert!(describe(&model(false, false, None)).contains("价格未知"));
        assert!(describe(&model(false, true, Some(2.0))).contains("200K 上下文"));
    }
    #[tokio::test]
    async fn opencode_asks_for_a_model_while_codex_only_asks_for_a_launcher() {
        let mut wizard = Wizard::new(Settings::default());
        assert!(wizard.accept("opencode".into()).await.unwrap().is_none());
        assert_eq!(wizard.field().key, "OPENCODE_BIN");
        assert!(wizard.accept("opencode".into()).await.unwrap().is_none());
        assert_eq!(wizard.field().key, "OPENCODE_MODEL");
        let mut wizard = Wizard::new(Settings::default());
        assert!(wizard.accept("codex".into()).await.unwrap().is_none());
        assert_eq!(wizard.field().key, "CODEX_BIN");
        assert!(wizard.accept("codex".into()).await.unwrap().is_none());
        // Runner, launcher, environment: three prompts, then the wizard is done.
        assert!(wizard.accept(String::new()).await.unwrap().is_some());
        assert!(
            Wizard::new(Settings::default())
                .accept("nope".into())
                .await
                .is_err()
        );
    }
    #[test]
    fn only_listed_indices_on_the_model_step_resolve_to_a_model() {
        let mut settings = Settings::default();
        settings.set("ADMIN_AGENT_PROVIDER", "opencode".into());
        let mut wizard = Wizard::new(settings);
        // Plan is runner, launcher, model: the model prompt is the third one.
        wizard.step = 2;
        wizard.catalog = Some(Ok(vec![OpenCodeModel {
            id: "opencode/free".into(),
            name: "Free".into(),
            free: true,
            input: Some(0.0),
            output: Some(0.0),
            context: None,
            tools: true,
            status: "active".into(),
            priced: true,
        }]));
        let model = wizard.field();
        assert_eq!(
            wizard.chosen_model(&model, "1").as_deref(),
            Some("opencode/free")
        );
        for out_of_range in ["0", "2", "", "-1", "free"] {
            assert_eq!(wizard.chosen_model(&model, out_of_range), None);
        }
        // A number typed anywhere but the model step stays a literal value.
        wizard.step = 1;
        let launcher = wizard.field();
        assert_eq!(wizard.chosen_model(&launcher, "1"), None);
    }
}
