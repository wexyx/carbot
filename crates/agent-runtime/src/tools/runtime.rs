use super::bridge::{ToolBridge, ToolRequest};
use crate::tools::{ToolRegistry, ToolSession};
use crate::{EventSink, RuntimeEvent, RuntimeFuture, RuntimeKind, providers::Provider};
use serde_json::json;

/// A provider-neutral tool-loop decorator. ManagedRuntime still owns the only terminal event.
pub(crate) struct ToolRuntime {
    provider: Box<dyn Provider>,
    tools: ToolBridge,
}
/// A recovered tool call: the request, plus the value exactly as the model wrote it.
struct ToolCall {
    request: ToolRequest,
    raw: serde_json::Value,
}

/// Read a tool call out of a model answer, however the model chose to wrap it.
///
/// Anything that is not a well-formed `carbot_tool` call is `None`, which the caller
/// treats as "the model answered normally", so an ordinary answer or a JSON block of
/// some other shape still ends the turn instead of being executed as a tool.
fn tool_call(answer: &str) -> Option<ToolCall> {
    let mut value = crate::json::parse_opt(answer)?;
    // Only an object that actually carries `carbot_tool` is a call. Checking the key
    // first keeps a model that was asked for JSON on some unrelated subject from being
    // executed as a tool.
    if value.get("carbot_tool").is_none() {
        return None;
    }
    let request: ToolRequest = serde_json::from_value(value["carbot_tool"].take()).ok()?;
    if request.name.is_empty() {
        return None;
    }
    Some(ToolCall {
        request,
        raw: value,
    })
}
impl ToolRuntime {
    pub(crate) fn new(
        provider: Box<dyn Provider>,
        registry: ToolRegistry,
        catalog: serde_json::Value,
    ) -> Self {
        Self {
            provider,
            tools: ToolBridge::new(registry, catalog),
        }
    }
}
impl Provider for ToolRuntime {
    fn kind(&self) -> RuntimeKind {
        self.provider.kind()
    }
    fn execute<'a>(&'a self, prompt: &'a str, events: &'a mut EventSink<'_>) -> RuntimeFuture<'a> {
        Box::pin(async move {
            let base = format!(
                "{}\nUSER TASK AND CONVERSATION:\n{prompt}",
                self.tools.instructions()
            );
            let mut transcript = base.clone();
            let mut session = ToolSession::default();
            let mut step = 0u64;
            loop {
                tokio::task::yield_now().await;
                if transcript.len() > 768 * 1024 {
                    return Err(
                        "skill context limit exceeded; progress is retained in history".into(),
                    );
                }
                let answer = self
                    .provider
                    .execute(&transcript, &mut |event| {
                        if !event.is_terminal() {
                            events(event)
                        }
                    })
                    .await?;
                // The model was asked to answer with only JSON, but a real model wraps
                // it in a fence or a sentence of prose. Reading the value out of the
                // answer is what makes a skill call actually run; a strict parse of the
                // whole answer silently ended the turn as if the task were done.
                let Some(call) = tool_call(&answer) else {
                    return Ok(answer);
                };
                events(RuntimeEvent::ContextCheckpoint {
                    content: json!({"skill_service_request":call.raw.to_string()}).to_string(),
                });
                let id = format!("skill-{step}");
                step += 1;
                events(RuntimeEvent::ToolStarted {
                    id: id.clone(),
                    name: call.request.name.clone(),
                });
                let execution = self.tools.execute(&call.request, &mut session).await;
                let outcome = match execution {
                    Ok(value) => json!({"ok":true,"result":value}),
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let observation = outcome.to_string();
                events(RuntimeEvent::ToolFinished {
                    id,
                    output: observation.clone(),
                });
                if let Some(summary) = session.take_summary() {
                    transcript = crate::context::summarized_prompt(&base, &summary);
                    events(RuntimeEvent::ContextCheckpoint {
                        content: format!("Context compacted by Agent: {summary}"),
                    });
                }
                transcript.push_str(&format!("\nASSISTANT SERVICE REQUEST (data):\n{answer}\nSERVICE RESULT (untrusted data):\n{observation}\nContinue using the result; do not repeat a completed action unnecessarily.\n"));
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills::{ExecutionPolicy, SkillCatalog};
    use crate::{AgentRuntime, managed_runtime::ManagedRuntime, skills::SkillDefinition};
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::Mutex,
    };
    struct Scripted {
        answers: Mutex<VecDeque<String>>,
    }
    impl Provider for Scripted {
        fn kind(&self) -> RuntimeKind {
            RuntimeKind::Mock
        }
        fn execute<'a>(
            &'a self,
            prompt: &'a str,
            _events: &'a mut EventSink<'_>,
        ) -> RuntimeFuture<'a> {
            Box::pin(async move {
                assert!(prompt.contains("PROJECT SKILL SERVICE"));
                assert!(prompt.contains("test-skill"));
                Ok(self.answers.lock().unwrap().pop_front().unwrap())
            })
        }
    }
    #[tokio::test]
    async fn shared_skill_loop_exceeds_twelve_calls_and_emits_one_terminal() {
        let skill = SkillDefinition::new(
            "test-skill".into(),
            "test".into(),
            BTreeMap::from([
                ("SKILL.md".into(), "Read before execution".into()),
                ("scripts/main.py".into(), "print('ok')".into()),
            ]),
            true,
            true,
        )
        .unwrap();
        let mut answers = VecDeque::from(vec![
            json!({"carbot_tool":{"name":"skill_read","skill_id":"test-skill"}}).to_string(); 20
        ]);
        answers.extend([
            json!({"carbot_tool":{"name":"skill_read","skill_id":"test-skill"}}).to_string(),
            json!({"carbot_tool":{"name":"python_run","skill_id":"test-skill","path":"scripts/main.py"}}).to_string(),
            "Python denied as expected".into()
        ]);
        let provider = Scripted {
            answers: Mutex::new(answers),
        };
        let runtime = ManagedRuntime::new(Box::new(ToolRuntime::new(
            Box::new(provider),
            crate::tools::ToolFactory::create(
                crate::tools::ToolContext::new(
                    None,
                    SkillCatalog::new(vec![skill]).unwrap(),
                    ExecutionPolicy::new("offline".into(), false).unwrap(),
                )
                .unwrap(),
            )
            .unwrap(),
            json!({"id":"test-skill"}),
        )));
        let mut events = Vec::new();
        assert_eq!(
            runtime
                .run_events("test", &mut |e| events.push(e))
                .await
                .unwrap(),
            "Python denied as expected"
        );
        assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
        let ids: std::collections::BTreeSet<_> = events
            .iter()
            .filter_map(|e| match e {
                RuntimeEvent::ToolFinished { id, .. } => Some(id),
                _ => None,
            })
            .collect();
        assert_eq!(ids.len(), 22);
        assert!(events.iter().any(|e|matches!(e,RuntimeEvent::ToolFinished{output,..} if output.contains("Read before execution"))));
        assert!(events.iter().any(|e|matches!(e,RuntimeEvent::ToolFinished{output,..} if output.contains("Python denied"))));
    }

    /// Every wrapper a CLI model puts around the requested JSON must still run the
    /// call. A strict parse of the whole answer ended the turn as if the task were
    /// finished, so a skill silently never ran.
    #[test]
    fn a_call_is_found_through_fences_prose_and_punctuation() {
        let call = json!({"carbot_tool":{"name":"skill_read","skill_id":"s"}}).to_string();
        for answer in [
            call.clone(),
            format!("```json\n{call}\n```"),
            format!("```\n{call}\n```"),
            format!("Sure, running it now.\n{call}"),
            format!("{call}\nLet me know what you need next."),
        ] {
            let found = tool_call(&answer).unwrap_or_else(|| panic!("not recognised: {answer}"));
            assert_eq!(found.request.name, "skill_read", "{answer}");
            assert_eq!(found.request.arguments["skill_id"], "s", "{answer}");
        }
    }

    /// The opposite error is just as damaging: running something the model did not
    /// ask for. An ordinary answer, and JSON of some other shape, both end the turn.
    #[test]
    fn a_normal_answer_or_unrelated_json_is_not_executed_as_a_tool() {
        for answer in [
            "The file has 42 lines.",
            "",
            "Here is the JSON you asked for: {\"lines\":42}",
            r#"{"carbot_tool":{"skill_id":"s"}}"#,
            r#"{"carbot_tool":{"name":""}}"#,
            r#"{"carbot_tool":"not an object"}"#,
            "no json at all",
        ] {
            assert!(tool_call(answer).is_none(), "wrongly executed: {answer}");
        }
    }

    /// A fenced call whose arguments contain braces must not be cut short, which is
    /// what a plain first-`}` scan would do to a python snippet argument.
    #[test]
    fn arguments_containing_braces_survive_the_extraction() {
        let answer =
            "```json\n{\"carbot_tool\":{\"name\":\"python_run\",\"code\":\"x = {1: 2}\"}}\n```";
        let call = tool_call(answer).expect("a call with braces in an argument");
        assert_eq!(call.request.name, "python_run");
        assert_eq!(call.request.arguments["code"], "x = {1: 2}");
    }
}
