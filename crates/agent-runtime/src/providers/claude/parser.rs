//! Vendor event names stay at this boundary, never in AgentRuntime or gateway code.
use crate::{
    EventSink, RuntimeEvent,
    errors::{is_token_insufficient, token_limit_error},
};
use serde_json::Value;

fn failure(value: &Value) -> String {
    let message = value
        .pointer("/error/message")
        .and_then(Value::as_str)
        .or_else(|| value["message"].as_str())
        .or_else(|| value["result"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| value.to_string());
    if is_token_insufficient(&value.to_string()) {
        token_limit_error(&message)
    } else {
        message
    }
}

#[derive(Default)]
pub(super) struct EventParser {
    pub answer: String,
    pub error: Option<String>,
    finished: bool,
}
impl EventParser {
    pub fn consume(&mut self, value: &Value, events: &mut EventSink<'_>) {
        if self.finished {
            return;
        }
        let kind = value["type"].as_str().unwrap_or_default();
        if kind == "error" || (kind == "result" && value["is_error"] == true) {
            self.error = Some(failure(value));
            self.finished = true;
            return;
        }
        if [
            "/event/message/stop_reason",
            "/event/delta/stop_reason",
            "/message/stop_reason",
        ]
        .iter()
        .any(|p| value.pointer(p).and_then(Value::as_str) == Some("max_tokens"))
        {
            self.error = Some(token_limit_error(
                "Claude Code reached a token/context limit",
            ));
            self.finished = true;
            return;
        }
        match kind {
            "assistant" | "user" => events(RuntimeEvent::ContextCheckpoint {
                content: value.to_string(),
            }),
            "stream_event" => {
                if let Some(text) = value.pointer("/event/delta/text").and_then(Value::as_str) {
                    self.answer.push_str(text);
                    events(RuntimeEvent::TextDelta { text: text.into() });
                }
            }
            "result" => {
                // Final result is authoritative; streaming already emitted text must not be repeated.
                if let Some(text) = value["result"].as_str() {
                    if let Some(suffix) = text.strip_prefix(&self.answer) {
                        if !suffix.is_empty() {
                            events(RuntimeEvent::TextDelta {
                                text: suffix.into(),
                            });
                        }
                    }
                    self.answer = text.into();
                }
                self.finished = true;
            }
            _ => (),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn claude_reconciles_final_text_and_ignores_late_events() {
        let mut parser = EventParser::default();
        let mut output = Vec::new();
        for value in [
            json!({"type":"stream_event","event":{"delta":{"text":"hel"}}}),
            json!({"type":"result","result":"hello"}),
            json!({"type":"stream_event","event":{"delta":{"text":"late"}}}),
        ] {
            parser.consume(&value, &mut |e| output.push(e));
        }
        assert_eq!(parser.answer, "hello");
        assert_eq!(
            output,
            vec![
                RuntimeEvent::TextDelta { text: "hel".into() },
                RuntimeEvent::TextDelta { text: "lo".into() }
            ]
        );
        assert!(!output.iter().any(RuntimeEvent::is_terminal));
    }
    #[test]
    fn claude_error_result_is_failure_without_requiring_nonzero_exit() {
        let mut parser = EventParser::default();
        parser.consume(
            &json!({"type":"result","is_error":true,"result":"authentication failed"}),
            &mut |_| panic!("error text must not be a delta"),
        );
        assert_eq!(parser.error.as_deref(), Some("authentication failed"));
    }
    #[test]
    fn claude_stop_reason_is_classified() {
        let mut parser = EventParser::default();
        parser.consume(
            &json!({"type":"stream_event","event":{"delta":{"stop_reason":"max_tokens"}}}),
            &mut |_| {},
        );
        assert!(is_token_insufficient(parser.error.as_ref().unwrap()));
    }
}
