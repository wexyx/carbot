//! Supported provider identities and configuration-name compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeKind {
    Mock,
    Carbot,
    Claude,
    Codex,
}
impl RuntimeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mock => "mock",
            Self::Carbot => "carbot",
            Self::Claude => "claude",
            Self::Codex => "codex",
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
            other => Err(format!(
                "Unknown AGENT_PROVIDER '{other}'; choose carbot, claude, codex, or mock"
            )),
        }
    }
}
