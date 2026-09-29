//! Provider lifecycle adapter: normalize progress and publish one final outcome.
use crate::{RuntimeErrorCode, RuntimeEvent, providers::Provider};

pub(crate) struct ManagedRuntime {
    provider: Box<dyn Provider>,
    attachment_root: std::path::PathBuf,
    _workspace: Option<tempfile::TempDir>,
    environment: crate::environment::AgentEnvironment,
}
impl ManagedRuntime {
    pub(crate) fn new(provider: Box<dyn Provider>) -> Self {
        Self {
            provider,
            attachment_root: crate::config::workdir(),
            _workspace: None,
            environment: Default::default(),
        }
    }
    pub(crate) fn with_attachment_root(mut self, root: std::path::PathBuf) -> Self {
        self.attachment_root = root;
        self
    }
    pub(crate) fn with_environment(
        mut self,
        environment: crate::environment::AgentEnvironment,
    ) -> Self {
        self.environment = environment;
        self
    }
    pub(crate) fn retain_workspace(mut self, workspace: Option<tempfile::TempDir>) -> Self {
        self._workspace = workspace;
        self
    }
}
impl crate::AgentRuntime for ManagedRuntime {
    fn kind(&self) -> crate::RuntimeKind {
        self.provider.kind()
    }
    fn run_events<'a>(
        &'a self,
        prompt: &'a str,
        events: &'a mut crate::EventSink<'_>,
    ) -> crate::RuntimeFuture<'a> {
        Box::pin(async move {
            let result = self
                .environment
                .scope(async {
                    let source = prompt.to_owned();
                    let root = self.attachment_root.clone();
                    let (prompt, attachments) = tokio::task::spawn_blocking(move || {
                        crate::attachments::PreparedAttachments::prepare(
                            &crate::attachments::AttachmentStore::default(),
                            &source,
                            &root,
                        )
                    })
                    .await
                    .map_err(|e| e.to_string())??;
                    attachments
                        .scope(self.provider.execute(&prompt, &mut |event| {
                            // A provider must not publish an early terminal before process exit is checked.
                            if !event.is_terminal() {
                                events(event);
                            }
                        }))
                        .await
                })
                .await;
            events(match &result {
                Ok(text) => RuntimeEvent::Completed { text: text.clone() },
                Err(message) => RuntimeEvent::Failed {
                    code: if crate::is_token_insufficient(message) {
                        RuntimeErrorCode::TokenInsufficient
                    } else {
                        RuntimeErrorCode::ExecutionFailed
                    },
                    message: message.clone(),
                },
            });
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentRuntime, RuntimeFactory};
    struct Failing;
    impl Provider for Failing {
        fn kind(&self) -> crate::RuntimeKind {
            crate::RuntimeKind::Mock
        }
        fn execute<'a>(
            &'a self,
            _: &'a str,
            events: &'a mut crate::EventSink<'_>,
        ) -> crate::RuntimeFuture<'a> {
            Box::pin(async move {
                events(RuntimeEvent::Completed {
                    text: "premature".into(),
                });
                events(RuntimeEvent::TextDelta {
                    text: "partial".into(),
                });
                Err("TOKEN_INSUFFICIENT: fixture".into())
            })
        }
    }
    #[tokio::test]
    async fn failure_has_one_authoritative_terminal_after_progress() {
        let runtime = ManagedRuntime::new(Box::new(Failing));
        let mut events = Vec::new();
        assert!(
            runtime
                .run_events("task", &mut |e| events.push(e))
                .await
                .is_err()
        );
        assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
        assert!(matches!(
            events.last(),
            Some(RuntimeEvent::Failed {
                code: RuntimeErrorCode::TokenInsufficient,
                ..
            })
        ));
        assert!(matches!(
            events.first(),
            Some(RuntimeEvent::TextDelta { .. })
        ));
    }
    #[tokio::test]
    async fn success_has_exactly_one_terminal_and_serializable_events() {
        let runtime = RuntimeFactory::create("mock").unwrap();
        let mut events = Vec::new();
        let answer = runtime
            .run_events("你好", &mut |e| events.push(e))
            .await
            .unwrap();
        assert_eq!(events.iter().filter(|e| e.is_terminal()).count(), 1);
        assert_eq!(
            events.last(),
            Some(&RuntimeEvent::Completed { text: answer })
        );
        for event in events {
            assert_eq!(
                serde_json::from_str::<RuntimeEvent>(&serde_json::to_string(&event).unwrap())
                    .unwrap(),
                event
            );
        }
    }
}
