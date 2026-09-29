pub fn is_token_insufficient(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    [
        "token_insufficient:",
        "context window",
        "context length",
        "context_length_exceeded",
        "maximum context",
        "too many tokens",
        "token limit",
        "max_tokens",
        "max tokens",
        "input too long",
        "prompt is too long",
        "insufficient tokens",
        "not enough tokens",
    ]
    .iter()
    .any(|marker| error.contains(marker))
}

pub(crate) fn token_limit_error(message: &str) -> String {
    format!("TOKEN_INSUFFICIENT: {message}")
}
