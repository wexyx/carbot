use crate::storage::ChatLog;
use agent_runtime::context::{HistoryFuture, HistorySource};
use uuid::Uuid;
pub(crate) struct LogHistory {
    logs: ChatLog,
    project: Uuid,
    chat: String,
}
impl LogHistory {
    pub(crate) fn new(logs: ChatLog, project: Uuid, chat: String) -> Self {
        Self {
            logs,
            project,
            chat,
        }
    }
}
impl HistorySource for LogHistory {
    fn read<'a>(&'a self, file: Option<String>, from_line: u64, to_line: u64) -> HistoryFuture<'a> {
        Box::pin(
            self.logs
                .read_lines(self.project, self.chat.clone(), file, from_line, to_line),
        )
    }
}
