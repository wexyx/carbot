use super::Manager;
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;

/// Application operations shared by the CLI and HTTP adapters. No transport or provider logic.
impl Manager {
    pub(crate) async fn workbench(
        self: &Arc<Self>,
        p: Uuid,
        op: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.core().project(p).await?;
        match op {
            "agents.list" => self.core().agent_candidates(p).await,
            "agents.show" => {
                let directory = self.core().agent_candidates(p).await?;
                directory["agents"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .find(|a| a["id"] == input["id"])
                    .cloned()
                    .ok_or("Agent 不存在或远端未连接".into())
            }
            "agents.save" => self.core().save_agent(p, input).await,
            "agents.virtual" => self.core().save_virtual_agent(p, input).await,
            "agents.start" => {
                let id = field(&input, "id")?;
                let row = self
                    .core()
                    .state()
                    .store
                    .get("local_agents", &format!("{p}:{id}"))
                    .await
                    .ok_or("not a configured local Agent")?;
                self.core().agent_start(p, row).await
            }
            "agents.stop" => self.core().propose_stop(p, field(&input, "id")?).await,
            "agents.delete" => {
                self.core()
                    .delete_agent(
                        p,
                        field(&input, "id")?,
                        input["expected_version"]
                            .as_u64()
                            .ok_or("expected_version required")?,
                    )
                    .await
            }
            "agents.test" => self.core().test_agent(p, input).await,
            "connections.list" => self.core().connections(p).await,
            "connections.configure" => self.core().configure_connection(p, input).await,
            "connections.connect" => self.core().propose_mount(p, input).await,
            "projects.list" => {
                let mut groups = vec![];
                for namespace in self.core().state().store.list("projects").await {
                    let project = field(&namespace, "id")?
                        .parse::<Uuid>()
                        .map_err(|e| e.to_string())?;
                    for group in self
                        .core()
                        .state()
                        .policy_store
                        .list(project, "group")
                        .await?
                    {
                        if group.body["kind"] == "agent_test" {
                            continue;
                        }
                        let mut row = json!(group);
                        row["namespace_id"] = json!(project);
                        groups.push(row);
                    }
                }
                groups.sort_by(|a, b| a["key"].as_str().cmp(&b["key"].as_str()));
                Ok(json!(groups))
            }
            "projects.delete" => self.core().delete_chat(p, input).await,
            "projects.rename" => self.core().rename_chat(p, input).await,
            "projects.create" => self.core().control(p, "group.create", input).await,
            "projects.configure" => {
                self.core()
                    .configure_group(p, field(&input, "id")?, input.clone())
                    .await
            }
            "capabilities.list" => {
                self.capability_rows(
                    p,
                    field(&input, "scope")?,
                    field(&input, "kind")?,
                    input["agent"].as_str().unwrap_or(""),
                    input["group"].as_str(),
                )
                .await
            }
            "capabilities.save" => {
                self.capability_save(
                    p,
                    field(&input, "scope")?,
                    field(&input, "kind")?,
                    input["body"].clone(),
                )
                .await
            }
            "capabilities.bind" => {
                self.capability_bind(
                    p,
                    field(&input, "scope")?,
                    field(&input, "kind")?,
                    input["agent"].as_str().unwrap_or(""),
                    input["group"].as_str(),
                    input["body"].clone(),
                )
                .await
            }
            "skills.list" => self.list_skills(p, field(&input, "scope")?).await,
            "skills.save" => {
                self.save_skill(p, field(&input, "scope")?, input["body"].clone())
                    .await
            }
            "tools.list" => {
                self.tool_settings(p, field(&input, "scope")?, field(&input, "agent")?)
                    .await
            }
            "tools.save" => {
                self.save_tool_settings(
                    p,
                    field(&input, "scope")?,
                    field(&input, "agent")?,
                    input["body"].clone(),
                )
                .await
            }
            "tools.test" => {
                self.start_tool_test(
                    p,
                    field(&input, "scope")?,
                    field(&input, "agent")?,
                    input["body"].clone(),
                )
                .await
            }
            "tools.test-status" => {
                self.tool_test(
                    p,
                    field(&input, "id")?
                        .parse()
                        .map_err(|_| "invalid test ID")?,
                )
                .await
            }
            "tools.test-cancel" => {
                self.cancel_tool_test(
                    p,
                    field(&input, "id")?
                        .parse()
                        .map_err(|_| "invalid test ID")?,
                )
                .await
            }
            "server.status" => Ok(json!({"url":self.web().address().await})),
            "server.start" => self.start_server(input).await,
            "server.stop" => {
                self.web().stop().await;
                Ok(json!({"status":"stopped"}))
            }
            _ => Err(format!("unknown workbench operation: {op}")),
        }
    }
}
fn field<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("{key} required"))
}
