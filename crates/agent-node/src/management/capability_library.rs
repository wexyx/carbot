use super::Manager;
use crate::capabilities::{Context, Library, Resource};
use serde_json::{Value, json};
use std::sync::Arc;
use uuid::Uuid;
impl Manager {
    pub(crate) async fn capability_context(
        &self,
        p: Uuid,
        scope: &str,
        agent: &str,
        group: Option<&str>,
    ) -> Result<Context, String> {
        self.core().project(p).await?;
        if scope == "management" {
            if !agent.is_empty() && agent != "admin" {
                return Err("invalid management Agent".into());
            }
        } else if scope == "business" {
            if !agent.is_empty()
                && !self
                    .core()
                    .state()
                    .store
                    .list("local_agents")
                    .await
                    .iter()
                    .any(|r| r["client_id"] == agent)
            {
                return Err("unknown local Agent".into());
            }
        } else {
            return Err("invalid capability scope".into());
        }
        if let Some(group) = group.filter(|g| !g.is_empty()) {
            if scope != "business" {
                return Err("management capabilities are node scoped".into());
            }
            self.core()
                .state()
                .policy_store
                .get(p, "group", group)
                .await?;
        }
        Ok(Context::new(p, agent).in_group(group))
    }
    pub(crate) async fn capability_rows(
        self: &Arc<Self>,
        p: Uuid,
        scope: &str,
        kind: &str,
        agent: &str,
        group: Option<&str>,
    ) -> Result<Value, String> {
        let context = self.capability_context(p, scope, agent, group).await?;
        let store = &self.core().state().store;
        let library = Library::new(store.clone());
        let mut resources = library.resources(scope, kind).await?;
        if kind == "tool" {
            let catalog = self.tool_catalog(p).await?;
            for definition in catalog[scope].as_array().into_iter().flatten() {
                let name = definition["name"].as_str().ok_or("tool name missing")?;
                resources.push(Resource {
                    id: format!("builtin:{scope}:tool:{name}"),
                    scope: scope.into(),
                    kind: kind.into(),
                    definition: definition.clone(),
                    version: 0,
                    readonly: true,
                    origin: None,
                });
            }
        }
        let legacy = store
            .get("tool_policies", &format!("{p}:{scope}:{agent}"))
            .await;
        let mut rows = vec![];
        for resource in resources {
            let fallback = if resource.readonly && kind == "tool" {
                Some(
                    !legacy
                        .as_ref()
                        .and_then(|r| r["policy"]["disabled"].as_array())
                        .is_some_and(|names| names.contains(&json!(resource.name()))),
                )
            } else {
                None
            };
            let resolution = context.resolve(store, &resource, fallback).await?;
            rows.push(json!({"resource":resource,"resolution":resolution}));
        }
        let agents = store.list("local_agents").await;
        let projects = store.list("projects").await;
        Ok(json!({"rows":rows,"agents":agents,"projects":projects,"context":context}))
    }
    pub(crate) async fn capability_save(
        self: &Arc<Self>,
        p: Uuid,
        scope: &str,
        kind: &str,
        input: Value,
    ) -> Result<Value, String> {
        self.core().project(p).await?;
        if kind == "tool" && input["deleted"] != true {
            let catalog = self.tool_catalog(p).await?;
            if catalog[scope]
                .as_array()
                .into_iter()
                .flatten()
                .any(|t| t["name"] == input["definition"]["name"])
            {
                return Err("cannot shadow built-in tool".into());
            }
        }
        Library::new(self.core().state().store.clone())
            .save(scope, kind, input)
            .await
    }
    pub(crate) async fn capability_bind(
        self: &Arc<Self>,
        p: Uuid,
        scope: &str,
        kind: &str,
        agent: &str,
        group: Option<&str>,
        input: Value,
    ) -> Result<Value, String> {
        let context = self.capability_context(p, scope, agent, group).await?;
        let id = input["id"].as_str().ok_or("resource id required")?;
        let rows = self.capability_rows(p, scope, kind, agent, group).await?;
        if !rows["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["resource"]["id"] == id)
        {
            return Err("unknown capability".into());
        }
        Library::new(self.core().state().store.clone())
            .bind(id, &context, input.clone())
            .await
    }
}
