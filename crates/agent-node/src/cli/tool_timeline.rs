use serde_json::Value;
#[derive(Default)]
pub(super) struct ToolTimeline {
    entries: Vec<Entry>,
}
struct Entry {
    marker: String,
    id: Option<String>,
    name: String,
    input: String,
    output: String,
    pending: bool,
    expanded: bool,
}
fn shown(v: &Value) -> String {
    if let Some(s) = v.as_str() {
        s.into()
    } else {
        serde_json::to_string_pretty(v).unwrap_or_default()
    }
}
impl ToolTimeline {
    pub fn record(&mut self, row: &Value) -> Option<String> {
        let event = row.get("payload").unwrap_or(row);
        let kind = event["type"].as_str().unwrap_or("");
        if !matches!(
            kind,
            "tool_started" | "agent.tool.started" | "tool_finished" | "agent.tool.finished"
        ) {
            return None;
        }
        let parsed = event["content"]
            .as_str()
            .or_else(|| event["text"].as_str())
            .and_then(|s| serde_json::from_str::<Value>(s).ok());
        let data = parsed.as_ref().unwrap_or(event);
        let id = data["call_id"]
            .as_str()
            .or_else(|| data["id"].as_str())
            .map(str::to_owned);
        if kind.ends_with("started") {
            let marker = format!("CARBOT_TOOL_{}", uuid::Uuid::new_v4().simple());
            self.entries.push(Entry {
                marker: marker.clone(),
                id,
                name: data["name"].as_str().unwrap_or("tool").into(),
                input: shown(
                    data.get("input")
                        .or_else(|| data.get("arguments"))
                        .unwrap_or(&Value::Null),
                ),
                output: String::new(),
                pending: true,
                expanded: false,
            });
            Some(String::new())
        } else {
            if let Some(entry) = self
                .entries
                .iter_mut()
                .find(|e| e.pending && id.as_ref().is_none_or(|id| e.id.as_ref() == Some(id)))
            {
                entry.output = shown(
                    data.get("output")
                        .or_else(|| data.get("content"))
                        .unwrap_or(data),
                );
                if entry.output.len() > 65536 {
                    let mut at = 65536;
                    while !entry.output.is_char_boundary(at) {
                        at -= 1
                    }
                    entry.output.truncate(at);
                    entry.output.push_str("\n…输出已截断");
                }
                entry.pending = false;
                return Some(format!("\n{}\n", entry.marker));
            }
            Some(String::new())
        }
    }
    #[cfg(test)]
    pub fn toggle(&mut self, index: usize) {
        if let Some(entry) = self.entries.get_mut(index) {
            entry.expanded = !entry.expanded;
        }
    }
    #[cfg(test)]
    pub fn toggle_latest(&mut self) {
        if let Some(index) = self.entries.len().checked_sub(1) {
            self.toggle(index)
        }
    }
    pub fn details(&self, index: Option<usize>) -> String {
        let Some((number, entry)) = index
            .and_then(|n| n.checked_sub(1))
            .or_else(|| self.entries.len().checked_sub(1))
            .and_then(|i| self.entries.get(i).map(|entry| (i + 1, entry)))
        else {
            return "暂无该工具调用记录。".into();
        };
        format!(
            "[工具 #{number}] {} · {}\n输入：\n{}\n输出：\n{}",
            entry.name,
            if entry.pending {
                "执行中"
            } else {
                "已完成"
            },
            entry.input,
            entry.output
        )
    }
    pub fn render(&self, transcript: &str, width: usize) -> String {
        let mut result = transcript.to_owned();
        for (index, entry) in self.entries.iter().enumerate() {
            if !result.contains(&entry.marker) {
                continue;
            }
            let summary = format!(
                "[工具 #{}] {} {} · {} · /tools 查看",
                index + 1,
                if entry.expanded { "▾" } else { "▸" },
                entry.name.split_whitespace().collect::<Vec<_>>().join(" "),
                if entry.pending {
                    "执行中"
                } else {
                    "已完成"
                }
            );
            let summary = super::screen::wrap(&summary, width)
                .first()
                .cloned()
                .unwrap_or_default();
            let detail = if entry.expanded {
                format!(
                    "\n\n输入\n\n{}\n\n输出\n\n{}\n",
                    entry.input,
                    if entry.pending {
                        "等待返回…"
                    } else {
                        &entry.output
                    }
                )
            } else {
                String::new()
            };
            result = result.replace(&entry.marker, &format!("{summary}{detail}"));
        }
        result
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn one_row_by_default_and_details_are_expandable() {
        let mut tools = ToolTimeline::default();
        tools.record(&json!({"type":"tool_started","name":"long_tool","call_id":"x","input":{"secret":"detail"}}));
        let line = tools
            .record(&json!({"type":"tool_finished","call_id":"x","output":"many\nlines"}))
            .unwrap();
        let compact = tools.render(&line, 80);
        assert!(compact.contains("已完成"));
        assert!(!compact.contains("many"));
        assert_eq!(compact.trim().lines().count(), 1);
        tools.toggle_latest();
        assert!(tools.render(&line, 80).contains("many\nlines"));
    }
}
