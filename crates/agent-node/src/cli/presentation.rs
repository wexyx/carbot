use serde_json::Value;

/// A view of durable events, not another conversation store.
#[derive(Default)]
pub(super) struct Presentation {
    text: String,
    streaming: bool,
    invocation: String,
}
impl Presentation {
    pub(super) fn command_result(value: &Value) -> String {
        if let Some(rows) = value.as_array() {
            return rows
                .iter()
                .map(|row| {
                    format!(
                        "{} · {}",
                        row["name"].as_str().unwrap_or("项目"),
                        row["id"].as_str().unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
        }
        if let Some(project) = value["project_id"].as_str() {
            return format!(
                "项目：{project}\n会话：{}",
                value["group"]
                    .as_str()
                    .or_else(|| value["admin_session"].as_str())
                    .unwrap_or_default()
            );
        }
        match value["status"].as_str() {
            Some("accepted" | "running") => "已发送，等待 Agent 回复…".into(),
            Some("interrupt_requested" | "interrupted") => "已请求打断当前任务。".into(),
            Some("denied") => "已拒绝该操作。".into(),
            Some(status) => status.into(),
            None if value.get("resolved").is_some() => "已处理目录访问确认。".into(),
            None if value.get("id").is_some() => "已发送，等待 Agent 回复…".into(),
            _ => "操作完成。".into(),
        }
    }
    pub(super) fn approval(item: &Value) -> String {
        let id = item["id"].as_str().unwrap_or_default();
        let targets = item["input"]["local_agents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "\n需要确认：{}\n目标：{} {}\n{}\n/approve {id} 或 /deny {id}",
            item["tool"].as_str().unwrap_or("管理操作"),
            item["client_id"]
                .as_str()
                .or_else(|| item["input"]["url"].as_str())
                .unwrap_or_default(),
            targets,
            item["warning"].as_str().unwrap_or_default()
        )
    }
    pub(super) fn render(&mut self, row: &Value) -> String {
        let event = row.get("payload").unwrap_or(row);
        let kind = event["type"].as_str().unwrap_or_default();
        let text = event["text"]
            .as_str()
            .or_else(|| event["content"].as_str())
            .unwrap_or_default();
        if event["aggregate"] == true {
            return String::new();
        }
        let invocation = event["invocation_id"].as_str().unwrap_or_default();
        let label = event["agent"]
            .as_str()
            .map(|s| if s == "default" { "默认 Agent" } else { s })
            .unwrap_or("Agent");
        if !invocation.is_empty() && self.invocation != invocation {
            self.invocation = invocation.into();
            self.streaming = false;
            self.text.clear();
        }
        match kind {
            "user" | "message.created" => {
                self.text.clear();
                self.streaming = false;
                String::new() // stdin already echoed the user's input
            }
            "text_delta" | "agent.delta" => {
                if text.is_empty() {
                    return String::new();
                }
                let prefix = if self.streaming {
                    String::new()
                } else {
                    format!("\n│ {label}\n")
                };
                self.streaming = true;
                self.text.push_str(text);
                format!("{prefix}{text}")
            }
            "tool_started" | "agent.tool.started" => {
                let name = event["name"]
                    .as_str()
                    .map(str::to_owned)
                    .or_else(|| {
                        serde_json::from_str::<Value>(text)
                            .ok()
                            .and_then(|v| v["name"].as_str().map(str::to_owned))
                    })
                    .unwrap_or_else(|| "tool".into());
                self.streaming = false;
                format!("\n[调用工具：{name}]\n")
            }
            "agent.message" => {
                let output = if self.text.ends_with(text) && self.streaming {
                    String::new()
                } else {
                    format!("\n│ {label}\n{text}\n")
                };
                self.streaming = false;
                self.text.clear();
                output
            }
            "completed" | "agent.done" => {
                let answer = if text.is_empty() || self.text.ends_with(text) {
                    String::new()
                } else {
                    format!("\n│ {label}\n{text}")
                };
                self.text.clear();
                self.streaming = false;
                format!("{answer}\n[完成]\n")
            }
            "failed" | "agent.error" | "task.interrupted" => {
                self.text.clear();
                self.streaming = false;
                format!(
                    "\n[结束] {}\n",
                    event["message"]
                        .as_str()
                        .or_else(|| event["error"].as_str())
                        .unwrap_or(text)
                )
            }
            // Tool results/checkpoints are retained in history, not dumped into chat.
            _ => String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn speaker_is_a_separate_header_for_streamed_and_complete_messages() {
        let mut view = Presentation::default();
        assert_eq!(view.render(&json!({"type":"agent.delta","agent":"default","invocation_id":"one","content":"你好"})), "\n│ 默认 Agent\n你好");
        assert_eq!(view.render(&json!({"type":"agent.delta","agent":"default","invocation_id":"one","content":"。"})), "。");
        assert_eq!(view.render(&json!({"type":"agent.message","agent":"reviewer","invocation_id":"two","content":"检查完成"})), "\n│ reviewer\n检查完成\n");
    }
    #[test]
    fn streams_fragments_once_and_does_not_dump_internal_events() {
        let mut view = Presentation::default();
        let mut output = String::new();
        for event in [
            json!({"type":"tool_started","name":"skill_read"}),
            json!({"type":"tool_finished","output":"long guide"}),
            json!({"type":"text_delta","text":"我在"}),
            json!({"type":"text_delta","text":"这里"}),
            json!({"type":"completed","text":"我在这里"}),
        ] {
            output.push_str(&view.render(&event));
        }
        assert_eq!(output.matches("我在这里").count(), 1);
        assert_eq!(output.matches("[完成]").count(), 1);
        assert!(!output.contains("long guide") && !output.contains("text_delta"));
        assert!(
            view.render(&json!({"type":"completed","text":"无流式片段的回答"}))
                .contains("无流式片段的回答")
        );
    }
}
