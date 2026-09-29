use crate::{AppState, event};
use serde_json::json;
use uuid::Uuid;
tokio::task_local! { static NESTED: bool; static PLANNING: bool; }

pub(super) struct MemberEvents {
    state: AppState,
    project: Uuid,
    run: Option<Uuid>,
    invocation: Uuid,
    agent: String,
    role: String,
    planning: bool,
}
impl MemberEvents {
    pub(super) fn new(state: &AppState, project: Uuid, member: &super::policies::Member) -> Self {
        Self {
            state: state.clone(),
            project,
            run: if NESTED.try_with(|v| *v).unwrap_or(false) {
                None
            } else {
                super::project_execution::ProjectExecution::run()
            },
            invocation: Uuid::new_v4(),
            agent: member.path.join("/"),
            role: member.role.clone(),
            planning: PLANNING.try_with(|v| *v).unwrap_or(false),
        }
    }
    pub(super) async fn scope<F: std::future::Future>(&self, future: F) -> F::Output {
        NESTED.scope(true, future).await
    }
    pub(super) async fn planning<F: std::future::Future>(future: F) -> F::Output {
        PLANNING.scope(true, future).await
    }
    pub(super) async fn emit(&self, kind: &str, content: &str) {
        let Some(run) = self.run else { return };
        let kind = if self.planning && matches!(kind, "agent.delta" | "agent.message") {
            "agent.planning"
        } else {
            kind
        };
        let output = event(
            run,
            kind,
            json!({"message_id":run,"invocation_id":self.invocation,"agent":self.agent,"role":self.role,"content":content}),
        );
        if crate::persist_event(
            &self.state,
            self.project,
            &format!("session:{run}"),
            &output,
        )
        .await
        {
            if let Some(session) = self.state.sessions.lock().await.get(&run) {
                let _ = session.events.send(output);
            }
        }
    }
}
