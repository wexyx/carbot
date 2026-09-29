//! Vendor event names stay at this boundary, never in AgentRuntime or gateway code.
use crate::{
    EventSink, RuntimeEvent,
    errors::{is_token_insufficient, token_limit_error},
};
use serde_json::Value;
use std::collections::HashSet;

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
    pending: String,
    completed: HashSet<String>,
    finished: bool,
}
impl EventParser {
    pub fn consume(&mut self, value: &Value, events: &mut EventSink<'_>) {
        if self.finished {
            return;
        }
        match value["type"].as_str().unwrap_or_default() {
            "turn.failed" | "error" => {
                self.error = Some(failure(value));
                self.finished = true;
            }
            "turn.completed" => self.finished = true,
            "agent_message_delta" => {
                if let Some(text) = value["delta"].as_str() {
                    self.pending.push_str(text);
                    self.answer.push_str(text);
                    events(RuntimeEvent::TextDelta { text: text.into() });
                }
            }
            "item.completed" if value["item"]["type"] == "agent_message" => {
                let item = &value["item"];
                if let Some(id) = item["id"].as_str() {
                    if !self.completed.insert(id.into()) {
                        return;
                    }
                }
                if let Some(text) = item["text"].as_str() {
                    if let Some(suffix) = text.strip_prefix(&self.pending) {
                        self.answer.push_str(suffix);
                        if !suffix.is_empty() {
                            events(RuntimeEvent::TextDelta {
                                text: suffix.into(),
                            });
                        }
                    } else {
                        // Correct a provider snapshot without pretending the correction is a delta.
                        self.answer.truncate(self.answer.len() - self.pending.len());
                        self.answer.push_str(text);
                    }
                    self.pending.clear();
                }
            }
            "item.started" | "item.updated" | "item.completed"
                if value["item"]["type"] != "agent_message" =>
            {
                events(RuntimeEvent::ContextCheckpoint {
                    content: value.to_string(),
                });
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
    fn cli_observations_survive_without_inventing_tool_semantics() {
        let observation = json!({"type":"item.completed","item":{"id":"tool-1","type":"command_execution","aggregated_output":"existing file verified","exit_code":0}});
        let mut saved = Vec::new();
        EventParser::default().consume(&observation, &mut |e| saved.push(e));
        assert_eq!(
            saved,
            vec![RuntimeEvent::ContextCheckpoint {
                content: observation.to_string()
            }]
        );
        assert_eq!(saved[0].wire_progress().unwrap().0, "agent.context");
        assert_eq!(saved[0].legacy_delta(), None);
    }
    #[test]
    fn codex_deduplicates_by_item_identity_not_equal_text() {
        let mut parser = EventParser::default();
        let mut text = String::new();
        for value in [
            json!({"type":"item.started","item":{"id":"a","type":"agent_message","text":"hel"}}),
            json!({"type":"agent_message_delta","delta":"hel"}),
            json!({"type":"item.completed","item":{"id":"a","type":"agent_message","text":"hello"}}),
            json!({"type":"item.completed","item":{"id":"a","type":"agent_message","text":"hello"}}),
            json!({"type":"item.completed","item":{"id":"b","type":"agent_message","text":"hello"}}),
            json!({"type":"turn.completed"}),
            json!({"type":"agent_message_delta","delta":"late"}),
        ] {
            parser.consume(&value, &mut |e| {
                if let RuntimeEvent::TextDelta { text: t } = e {
                    text.push_str(&t)
                }
            });
        }
        assert_eq!(text, "hellohello");
        assert_eq!(parser.answer, text);
    }
    #[test]
    fn codex_ordinary_errors_are_not_silently_successful() {
        let mut parser = EventParser::default();
        parser.consume(
            &json!({"type":"turn.failed","error":{"message":"login required"}}),
            &mut |_| {},
        );
        parser.consume(
            &json!({"type":"agent_message_delta","delta":"late"}),
            &mut |_| panic!("late event"),
        );
        assert_eq!(parser.error.as_deref(), Some("login required"));
    }
}
