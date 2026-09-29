use crate::{EventSink, RuntimeFuture, RuntimeKind};

pub(crate) struct Runtime;

impl Runtime {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl crate::providers::Provider for Runtime {
    fn kind(&self) -> RuntimeKind {
        RuntimeKind::Mock
    }
    fn execute<'a>(
        &'a self,
        _prompt: &'a str,
        on_event: &'a mut EventSink<'_>,
    ) -> RuntimeFuture<'a> {
        Box::pin(async move {
            let answer = "已收到消息。\n\n这是 Mock Agent 的模拟回复：通信链路正常，未调用模型，也未执行任务。".to_owned();
            for part in answer.split_inclusive(['：', '。', '！', '？']) {
                on_event(crate::RuntimeEvent::TextDelta {
                    text: part.to_string(),
                });
            }
            Ok(answer)
        })
    }
}
