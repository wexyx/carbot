use super::{
    commands::{self, Command},
    presentation::Presentation,
};
use crate::management::Manager;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Default)]
pub(super) struct Action {
    pub(super) text: String,
    pub(super) navigate: bool,
    pub(super) replay: bool,
    pub(super) configure: bool,
    pub(super) exit: bool,
    pub(super) sent: bool,
}
#[derive(Clone)]
pub(super) struct Controller {
    manager: Arc<Manager>,
    project: Uuid,
    session: Uuid,
    group: Option<String>,
    previous: Option<Uuid>,
    navigation: Vec<(Uuid, Uuid, Option<String>, Option<Uuid>)>,
}
impl Controller {
    pub(super) async fn new(manager: Arc<Manager>) -> Result<Self, String> {
        let project = manager.core().bootstrap().await?;
        let group = super::default_project::ensure(&manager, project).await?;
        let session = serde_json::from_value(manager.create_session(project).await?["id"].clone())
            .map_err(|e| e.to_string())?;
        Ok(Self {
            manager,
            project,
            session,
            group: Some(group),
            previous: None,
            navigation: Vec::new(),
        })
    }
    pub(super) fn view(&self) -> (Uuid, Uuid, Option<String>) {
        (
            self.project,
            self.previous
                .filter(|_| self.group.is_some())
                .unwrap_or(self.session),
            self.group.clone(),
        )
    }
    pub(super) async fn session_info(&self) -> String {
        super::session_info::SessionInfo::load(&self.manager, self.project, self.group.as_deref())
            .await
            .unwrap_or_else(|_| "会话配置暂不可用".into())
    }
    pub(super) fn label(&self) -> &str {
        if self.group.is_some() {
            "group"
        } else {
            "admin"
        }
    }
    pub(super) fn heading(&self) -> String {
        format!(
            "{} · {} · 项目 {} · 会话 {}",
            std::env::var("CARBOT_INSTANCE").unwrap_or_else(|_| "default".into()),
            self.label(),
            self.group.as_deref().unwrap_or("管理"),
            &self.view().1.to_string()[..8]
        )
    }
    pub(super) async fn interrupt(&self) -> Result<(), String> {
        if let Some(id) = self.previous.filter(|_| self.group.is_some()) {
            self.manager.core().interrupt(self.project, id).await?;
        } else {
            self.manager.interrupt(self.project, self.session).await?;
        }
        Ok(())
    }
    pub(super) fn can_back(&self) -> bool {
        !self.navigation.is_empty()
    }
    pub(super) async fn execute(&mut self, line: &str) -> Result<Action, String> {
        if line.trim() == "/back" {
            let (project, session, group, previous) = self
                .navigation
                .pop()
                .ok_or("没有上一个聊天；用 /chat 选择项目。")?;
            self.project = project;
            self.session = session;
            self.group = group;
            self.previous = previous;
            return Ok(Action {
                navigate: true,
                replay: true,
                text: format!("已返回 · {}", self.heading()),
                ..Default::default()
            });
        }
        let before = (
            self.project,
            self.session,
            self.group.clone(),
            self.previous,
        );
        let action = self.execute_inner(line).await?;
        if action.navigate
            && (before.0 != self.project || before.1 != self.session || before.2 != self.group)
        {
            self.navigation.push(before);
        }
        Ok(action)
    }
    async fn execute_inner(&mut self, line: &str) -> Result<Action, String> {
        let mut action = Action::default();
        let mut command = commands::parse(line)?;
        let answer = match line.trim().to_lowercase().as_str() {
            "确认" | "允许" | "同意" | "yes" | "y" => Some(true),
            "拒绝" | "no" | "n" => Some(false),
            _ => None,
        };
        if let Some(allow) = answer {
            let mut pending = self
                .manager
                .core()
                .approvals(self.project)
                .await?
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|row| {
                    super::permission_dialog::Permission::management(self.project, row)
                })
                .collect::<Vec<_>>();
            pending.extend(agent_runtime::workspace::pending().into_iter().map(|item| {
                super::permission_dialog::Permission {
                    id: item.id,
                    project: self.project,
                    workspace: true,
                    conversation: item.command.is_some() && item.conversation_id.is_some(),
                    title: item.operation,
                    detail: item.path,
                }
            }));
            if pending.len() > 1 {
                return Err("多个请求待确认；请使用 /approve ID 或 /allow-path ID 指定请求，或 Ctrl+P 打开确认界面".into());
            }
            if let Some(item) = pending.first() {
                command = commands::parse(&item.command(allow))?;
            }
        }
        let value = match command {
            Command::Allowlist(args) => {
                action.text = super::allowlist::execute(self.manager.core(), &args).await?;
                return Ok(action);
            }
            Command::Permissions(args) => {
                let parts = args.split_whitespace().collect::<Vec<_>>();
                let id = if let Some(id) = parts.first() {
                    id.to_string()
                } else if let Some(group) = &self.group {
                    let doc = self
                        .manager
                        .core()
                        .control(self.project, "group.get", serde_json::json!({"key":group}))
                        .await?;
                    let members = doc["body"]["policy"]["members"]
                        .as_array()
                        .ok_or("群成员不可用")?;
                    if members.len() != 1
                        || members[0]["path"].as_array().is_none_or(|p| p.len() != 1)
                    {
                        return Err(
                            "用法：/permissions AGENT_ID [ask|auto|full] [--confirm-full-access]"
                                .into(),
                        );
                    }
                    members[0]["path"][0].as_str().ok_or("成员不可用")?.into()
                } else {
                    "default".into()
                };
                if parts.len() > 3 || (parts.len() == 3 && parts[2] != "--confirm-full-access") {
                    return Err(
                        "用法：/permissions AGENT_ID [ask|auto|full] [--confirm-full-access]"
                            .into(),
                    );
                }
                let current = self.manager.core().permissions(self.project, &id).await?;
                let result = if let Some(mode) = parts.get(1) {
                    self.manager.core().set_permissions(self.project,&id,serde_json::json!({"mode":mode,"expected_version":current["version"],"confirm_full_access":parts.get(2)==Some(&"--confirm-full-access")})).await?
                } else {
                    current
                };
                action.text = format!(
                    "{} · {} · 对后续项目任务生效；管理聊天仍逐次审批。",
                    id,
                    result["label"].as_str().unwrap_or("")
                );
                return Ok(action);
            }
            Command::Workbench(op, input) => {
                if op == "agents.list" && input["contextual"] == true {
                    if let Some(group) = &self.group {
                        let result = self
                            .manager
                            .core()
                            .group_command(self.project, group, "/agents")
                            .await?;
                        action.text = result["message"].as_str().unwrap_or("").into();
                        return Ok(action);
                    }
                }
                let result = self.manager.workbench(self.project, &op, input).await?;
                if op == "agents.test" {
                    self.group = Some(result["key"].as_str().ok_or("missing test chat")?.into());
                    self.previous = None;
                    action.navigate = true;
                    action.replay = true;
                    action.text = "已进入 Agent 测试聊天；/manage 返回管理。".into();
                } else {
                    action.text = if result["status"] == "pending" {
                        Presentation::approval(&result)
                    } else {
                        super::workbench_view::render(&op, &result)
                    };
                }
                return Ok(action);
            }
            Command::Tools(index) => {
                let chat = self
                    .group
                    .as_ref()
                    .map(|g| format!("group:{g}"))
                    .unwrap_or_else(|| "admin".into());
                let rows = self
                    .manager
                    .core()
                    .state()
                    .store
                    .logs()
                    .read(self.project, chat, 0, u64::MAX, 1000)
                    .await?;
                let mut tools = super::tool_timeline::ToolTimeline::default();
                for row in rows {
                    tools.record(&row);
                }
                action.text = tools.details(index);
                return Ok(action);
            }
            Command::History(target) => {
                if target == "admin" {
                    self.group = None;
                    self.previous = None;
                } else if !target.is_empty() {
                    self.manager
                        .core()
                        .control(self.project, "group.get", serde_json::json!({"key":target}))
                        .await?;
                    self.group = Some(target);
                    self.previous = None;
                }
                action.navigate = true;
                action.replay = true;
                action.text = "已恢复最近的聊天记录；原始日志与上下文没有被删除。".into();
                return Ok(action);
            }
            Command::Exit => {
                action.exit = true;
                return Ok(action);
            }
            Command::Help(topic) => {
                action.text = super::help::topic(&topic)?.into();
                return Ok(action);
            }
            Command::AdminConfig => {
                self.manager.ensure_idle().await?;
                action.configure = true;
                return Ok(action);
            }
            Command::Interrupt => {
                self.interrupt().await?;
                action.text = "已请求打断当前任务。".into();
                return Ok(action);
            }
            Command::Confirm(id, allow) => {
                self.manager
                    .core()
                    .decide(
                        self.project,
                        id.parse().map_err(|_| "invalid approval UUID")?,
                        allow,
                    )
                    .await?
            }
            Command::WorkspaceConfirm(id, allow) => {
                agent_runtime::workspace::decide(
                    id.parse().map_err(|_| "invalid approval UUID")?,
                    allow,
                )?;
                serde_json::json!({"resolved":id})
            }
            Command::Group(line) => {
                let group = self
                    .group
                    .as_deref()
                    .ok_or("先用 /chat GROUP_ID 进入群聊")?;
                let result = self
                    .manager
                    .core()
                    .group_command(self.project, group, &line)
                    .await?;
                action.text = result["message"].as_str().unwrap_or("群配置已更新").into();
                return Ok(action);
            }
            Command::Say(content) => {
                let result = if let Some(group) = &self.group {
                    let result=self.manager.core().group_chat(self.project,serde_json::json!({"group_id":group,"content":content,"previous_session_id":self.previous})).await?;
                    self.previous = Some(
                        serde_json::from_value(result["id"].clone()).map_err(|e| e.to_string())?,
                    );
                    result
                } else {
                    self.manager
                        .message(self.project, self.session, content)
                        .await?
                };
                action.sent = true;
                result
            }
            Command::Project(None) => {
                serde_json::json!(self.manager.core().state().store.list("projects").await)
            }
            command => {
                match command {
                    Command::Project(Some(id)) => {
                        let project = id.parse().map_err(|_| "invalid project UUID")?;
                        self.manager.core().project(project).await?;
                        let session = serde_json::from_value(
                            self.manager.create_session(project).await?["id"].clone(),
                        )
                        .map_err(|e| e.to_string())?;
                        self.project = project;
                        self.session = session;
                        self.group = None;
                        self.previous = None;
                    }
                    Command::New => {
                        if let Some(group) = &self.group {
                            self.manager
                                .core()
                                .reset_group_context(self.project, group)
                                .await?;
                        } else {
                            self.manager
                                .reset_context(self.project, self.session)
                                .await?;
                        }
                        self.previous = None;
                        action.text = "已开启新上下文，历史日志仍然保留。".into();
                        return Ok(action);
                    }
                    Command::Resume(id) => {
                        action.replay = true;
                        let id = id.parse().map_err(|_| "invalid session UUID")?;
                        self.manager.history(self.project, id).await?;
                        self.session = id;
                        self.group = None;
                        self.previous = None;
                    }
                    Command::Chat(id) => {
                        let projects = self
                            .manager
                            .workbench(self.project, "projects.list", serde_json::json!({}))
                            .await?;
                        if let Some(row) = projects
                            .as_array()
                            .into_iter()
                            .flatten()
                            .find(|r| r["key"] == id)
                        {
                            self.project = serde_json::from_value(row["namespace_id"].clone())
                                .map_err(|e| e.to_string())?;
                            self.session = serde_json::from_value(
                                self.manager.create_session(self.project).await?["id"].clone(),
                            )
                            .map_err(|e| e.to_string())?;
                        }
                        self.manager
                            .core()
                            .control(self.project, "group.get", serde_json::json!({"key":id}))
                            .await?;
                        self.group = Some(id);
                        self.previous = None;
                    }
                    Command::Admin => {
                        self.group = None;
                        self.previous = None;
                    }
                    _ => unreachable!(),
                }
                action.navigate = true;
                action.text = self.heading();
                return Ok(action);
            }
        };
        action.text = Presentation::command_result(&value);
        Ok(action)
    }
}
