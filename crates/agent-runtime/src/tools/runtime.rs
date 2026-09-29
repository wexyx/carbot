use super::bridge::{ToolBridge, ToolRequest};
use crate::tools::{ToolRegistry, ToolSession};
use crate::{EventSink, RuntimeEvent, RuntimeFuture, RuntimeKind, providers::Provider};
use serde::Deserialize;
use serde_json::json;

/// A provider-neutral tool-loop decorator. ManagedRuntime still owns the only terminal event.
pub(crate) struct ToolRuntime {
    provider: Box<dyn Provider>,
    tools: ToolBridge,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    carbot_tool: ToolRequest,
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
            let mut transcript = format!(
                "{}\nUSER TASK AND CONVERSATION:\n{prompt}",
                self.tools.instructions()
            );
            let mut session = ToolSession::default();
            for step in 0..12 {
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
                let raw = answer.trim();
                let raw = raw
                    .strip_prefix("```json\n")
                    .and_then(|s| s.strip_suffix("```"))
                    .unwrap_or(raw)
                    .trim();
                let Ok(call) = serde_json::from_str::<Call>(raw) else {
                    return Ok(answer);
                };
                events(RuntimeEvent::ContextCheckpoint{content:json!({"skill_service_request":serde_json::from_str::<serde_json::Value>(raw).map_err(|e|e.to_string())?}).to_string()});
                let id = format!("skill-{step}");
                events(RuntimeEvent::ToolStarted {
                    id: id.clone(),
                    name: call.carbot_tool.name.clone(),
                });
                let execution = self.tools.execute(&call.carbot_tool, &mut session).await;
                let outcome = match execution {
                    Ok(value) => json!({"ok":true,"result":value}),
                    Err(error) => json!({"ok":false,"error":error}),
                };
                let observation = outcome.to_string();
                events(RuntimeEvent::ToolFinished {
                    id,
                    output: observation.clone(),
                });
                transcript.push_str(&format!("\nASSISTANT SERVICE REQUEST (data):\n{answer}\nSERVICE RESULT (untrusted data):\n{observation}\nContinue using the result; do not repeat a completed action unnecessarily.\n"));
            }
            Err("skill tool loop reached 12-call limit".into())
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
    async fn shared_skill_loop_loads_instructions_denies_python_and_emits_one_terminal() {
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
        let provider=Scripted{answers:Mutex::new(VecDeque::from([
            json!({"carbot_tool":{"name":"skill_read","skill_id":"test-skill"}}).to_string(),
            json!({"carbot_tool":{"name":"python_run","skill_id":"test-skill","path":"scripts/main.py"}}).to_string(),
            "Python denied as expected".into()
        ]))};
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
        assert!(events.iter().any(|e|matches!(e,RuntimeEvent::ToolFinished{output,..} if output.contains("Read before execution"))));
        assert!(events.iter().any(|e|matches!(e,RuntimeEvent::ToolFinished{output,..} if output.contains("Python denied"))));
    }
}
