//! Supported provider identities and configuration-name compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    Mock,
    Carbot,
    Claude,
    Codex,
    OpenCode,
}
impl RuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Carbot => "carbot",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCode => "opencode",
        }
    }
}
impl std::str::FromStr for RuntimeKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "mock" => Ok(Self::Mock),
            "carbot" | "builtin" => Ok(Self::Carbot),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencode" => Ok(Self::OpenCode),
            other => Err(format!(
                "Unknown AGENT_PROVIDER '{other}'; choose carbot, claude, codex, opencode, or mock"
            )),
        }
    }
}
