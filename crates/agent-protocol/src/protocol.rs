use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WireEvent {
    pub id: Uuid,
    #[serde(rename = "type")]
    pub kind: String,
    pub session_id: Uuid,
    #[serde(flatten)]
    pub data: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RouteContext {
    pub trace_id: Uuid,
    pub visited_nodes: Vec<String>,
}

impl Default for RouteContext {
    fn default() -> Self {
        Self {
            trace_id: Uuid::new_v4(),
            visited_nodes: Vec::new(),
        }
    }
}

impl RouteContext {
    pub fn enter(&mut self, node_id: &str) -> Result<(), &'static str> {
        if self.visited_nodes.iter().any(|id| id == node_id) {
            return Err("routing loop: this node already handled the task");
        }
        if self.visited_nodes.len() >= 8 {
            return Err("routing hop limit exceeded (maximum 8 nodes)");
        }
        self.visited_nodes.push(node_id.to_owned());
        Ok(())
    }
}

/// Decode whole JSON events before decoding UTF-8: network chunks can split a character.
#[derive(Default)]
pub struct SseDecoder {
    buffer: Vec<u8>,
    data: Vec<u8>,
}

impl SseDecoder {
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<WireEvent>, String> {
        self.push_json(chunk)?
            .into_iter()
            .map(|value| serde_json::from_value(value).map_err(|e| e.to_string()))
            .collect()
    }

    pub fn push_json(&mut self, chunk: &[u8]) -> Result<Vec<Value>, String> {
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        while let Some(end) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let mut line: Vec<u8> = self.buffer.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            if line.is_empty() {
                if !self.data.is_empty() {
                    if self.data != b"[DONE]" {
                        events.push(
                            serde_json::from_slice(&self.data)
                                .map_err(|e| format!("invalid SSE JSON: {e}"))?,
                        );
                    }
                    self.data.clear();
                }
            } else if let Some(data) = line.strip_prefix(b"data:") {
                if !self.data.is_empty() {
                    self.data.push(b'\n');
                }
                self.data
                    .extend_from_slice(data.strip_prefix(b" ").unwrap_or(data));
            }
            if self.data.len() > 1024 * 1024 {
                return Err("SSE event exceeds 1 MiB".into());
            }
        }
        if self.buffer.len() > 1024 * 1024 {
            return Err("SSE line exceeds 1 MiB".into());
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_split_unicode_and_crlf_frames() {
        let text = format!(
            "data: {{\"id\":\"{}\",\"session_id\":\"{}\",\"type\":\"command\",\"content\":\"跨环境任务\"}}\r\n\r\n",
            Uuid::new_v4(),
            Uuid::new_v4()
        );
        let mut decoder = SseDecoder::default();
        let mut events = Vec::new();
        for byte in text.bytes() {
            events.extend(decoder.push(&[byte]).unwrap());
        }
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].data["content"], "跨环境任务");
    }

    #[test]
    fn rejects_cycles_and_limits_hops() {
        let mut route = RouteContext::default();
        route.enter("a").unwrap();
        route.enter("b").unwrap();
        assert!(route.enter("a").is_err());
        for node in ["c", "d", "e", "f", "g", "h"] {
            route.enter(node).unwrap();
        }
        assert!(route.enter("i").is_err());
    }
}
