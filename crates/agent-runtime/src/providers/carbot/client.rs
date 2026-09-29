use super::{
    config::HarnessConfig,
    model_error::model_error,
    protocol::{ModelProtocol, ProtocolFactory},
    turn::Turn,
};
use crate::{RuntimeEvent, tools::ToolRegistry};
use agent_protocol::SseDecoder;
use futures_util::StreamExt;
use serde_json::Value;
use std::{collections::BTreeMap, time::Duration};
pub(super) struct ModelClient {
    http: reqwest::Client,
    config: HarnessConfig,
    protocol: Box<dyn ModelProtocol>,
}
impl ModelClient {
    pub(super) fn new(config: HarnessConfig) -> Result<Self, String> {
        let protocol = ProtocolFactory::create(config.api);
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            http,
            config,
            protocol,
        })
    }
    pub(super) async fn prepare_context(
        &self,
        history: &mut Vec<Value>,
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<(), String> {
        let image_reserve =
            crate::attachments::PreparedAttachments::images(|images| images.len() * 8192);
        let limit = self.config.context.input_limit(self.config.max_tokens)?.checked_sub(image_reserve).ok_or("CONTEXT_LIMIT: images exceed context length; attach fewer images or increase context length")?;
        let size = |rows: &[Value]| -> Result<usize, String> {
            let request = self
                .protocol
                .request(&self.http, &self.config, rows, tools)
                .build()
                .map_err(|e| e.to_string())?;
            Ok(request
                .body()
                .and_then(|b| b.as_bytes())
                .map_or(0, |b| b.len()))
        };
        let before = size(history)?;
        if before <= limit {
            return Ok(());
        }
        let fixed = size(&[])?;
        let mut available = limit
            .checked_sub(fixed + 1024)
            .ok_or("CONTEXT_LIMIT: system prompt and tool schemas exceed input budget")?;
        let sources = crate::context::HistoryAccess::manifest().await;
        if let Some(plan) = self
            .config
            .context
            .summary_plan(history, available.saturating_sub(sources.len()))?
        {
            events(RuntimeEvent::ContextCheckpoint {
                content: "正在智能压缩上下文…".into(),
            });
            let mut summary = String::new();
            let mut remaining = plan.older();
            while !remaining.is_empty() {
                let mut chunk_limit = (limit / 2).max(256);
                let (chunk, input) = loop {
                    let chunk = crate::context::SummaryPlan::chunk(remaining, chunk_limit);
                    let input = vec![
                        serde_json::json!({"role":"user","content":plan.summary_request(&summary,chunk)}),
                    ];
                    if size(&input)? <= limit {
                        break (chunk, input);
                    }
                    if chunk_limit <= 256 {
                        return Err("CONTEXT_LIMIT: 摘要请求无法容纳于上下文长度".into());
                    }
                    chunk_limit /= 2;
                };
                let turn = self
                    .request_turn(&input, &ToolRegistry::new(), &mut |_| {})
                    .await?;
                if !turn.calls().is_empty() {
                    return Err("智能压缩期间模型请求了工具，摘要未应用".into());
                }
                summary = plan.validate(turn.text())?;
                remaining = &remaining[chunk.len()..];
            }
            if summary.is_empty() {
                summary=plan.validate(r#"{"goals":[],"constraints":[],"decisions":[],"completed":[],"pending":[],"risks":[],"references":[]}"#)?;
            }
            let candidate = plan.finish(&summary, &sources);
            let after = size(&candidate)?;
            if after > limit {
                return Err("CONTEXT_LIMIT: 智能摘要仍超过上下文长度，原始上下文未修改".into());
            }
            *history = candidate;
            events(RuntimeEvent::ContextCheckpoint {
                content: format!(
                    "Context compacted: 智能压缩 {before} -> {after}; summary={summary}; {sources}"
                ),
            });
            return Ok(());
        }
        // JSON escaping can expand excerpts. Measure the actual request before sending.
        for _ in 0..6 {
            let candidate = self.config.context.compact(history, available)?;
            let after = size(&candidate)?;
            if after <= limit {
                *history = candidate;
                events(RuntimeEvent::ContextCheckpoint {
                    content: format!(
                        "Context compacted: estimated input bytes {before} -> {after}; original logs retained. Lossy local excerpts, not a model-generated summary."
                    ),
                });
                return Ok(());
            }
            available = available * 3 / 4;
        }
        Err("CONTEXT_LIMIT: unable to fit request; use /new or increase context budget".into())
    }
    pub(super) async fn next_turn(
        &self,
        history: &[Value],
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<Turn, String> {
        let history = self.protocol.with_images(history);
        self.request_turn(&history, tools, events).await
    }
    async fn request_turn(
        &self,
        history: &[Value],
        tools: &ToolRegistry,
        events: &mut (impl FnMut(RuntimeEvent) + Send),
    ) -> Result<Turn, String> {
        let response = self
            .protocol
            .request(&self.http, &self.config, history, tools)
            .send()
            .await
            .map_err(|_| "model connection failed")?;
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(model_error(&format!(
                "HTTP {status}: {}",
                text.chars().take(2000).collect::<String>()
            )));
        }
        let mut stream = response.bytes_stream();
        let mut decoder = SseDecoder::default();
        let mut turn = Turn::new();
        while let Some(chunk) = stream.next().await {
            for value in decoder.push_json(&chunk.map_err(|_| "model stream interrupted")?)? {
                if value.get("error").is_some()
                    || value["type"] == "error"
                    || value["type"] == "response.failed"
                {
                    return Err(model_error(&value.to_string()));
                }
                self.protocol.consume(&mut turn, value, &mut |text| {
                    events(RuntimeEvent::TextDelta { text })
                })?;
            }
            // Protocol completion, not TCP EOF, ends a turn. Some SSE servers keep
            // the connection open after finish_reason / message_delta / response.completed.
            if turn.is_complete() {
                break;
            }
        }
        turn.validate()?;
        Ok(turn)
    }
    pub(super) fn append_history(
        &self,
        history: &mut Vec<Value>,
        turn: &Turn,
        results: &BTreeMap<usize, String>,
    ) {
        self.protocol.append_history(history, turn, results);
    }
}
