use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn existing_connection(
        &self,
        project: Uuid,
        input: &Value,
    ) -> Result<Option<Value>, String> {
        let url = input["url"].as_str().ok_or("url required")?;
        let normalized = url::Url::parse(url)
            .map_err(|e| e.to_string())?
            .to_string()
            .trim_end_matches('/')
            .to_owned();
        Ok(self
            .connections(project)
            .await?
            .as_array()
            .unwrap()
            .iter()
            .find(|r| {
                r["direction"] == "upstream"
                    && r["url"].as_str().is_some_and(|u| {
                        url::Url::parse(u)
                            .is_ok_and(|u| u.to_string().trim_end_matches('/') == normalized)
                    })
            })
            .cloned())
    }
    pub(crate) async fn connections(&self, project: Uuid) -> Result<Value, String> {
        self.project(project).await?;
        let mut links = self.state().store.list("peer_mounts").await;
        links.extend(self.state().links.iter().map(|l| json!(l)));
        let mut rows = Vec::new();
        for link in links
            .iter()
            .filter(|r| r["local_project_id"] == json!(project))
        {
            let name = link["name"].as_str().unwrap_or_default();
            let config = self
                .state()
                .store
                .get("peer_link_settings", name)
                .await
                .unwrap_or(json!({}));
            if config["deleted"] == true {
                continue;
            }
            let disabled = config["disabled"] == true;
            let status = self
                .state()
                .link_status
                .lock()
                .await
                .get(name)
                .cloned()
                .unwrap_or("connecting".into());
            let status = if disabled {
                config["last_status"]
                    .as_str()
                    .unwrap_or("disconnected")
                    .to_owned()
            } else {
                status
            };
            rows.push(json!({"name":name,"direction":"upstream","url":link["url"],"status":status,"disabled":disabled}));
        }
        for peer in self
            .state()
            .store
            .list("peer_inbound")
            .await
            .into_iter()
            .filter(|r| r["project_id"] == json!(project))
        {
            let id = peer["client_id"].as_str().unwrap_or_default();
            let config = self
                .state()
                .store
                .get("peer_blocks", &format!("{project}:{id}"))
                .await
                .unwrap_or(json!({}));
            if config["deleted"] == true {
                continue;
            }
            let disabled = config["disabled"] == true;
            let online = self
                .state()
                .clients
                .lock()
                .await
                .get(&(project, id.into()))
                .is_some_and(|c| !c.sender.is_closed());
            rows.push(json!({"name":id,"direction":"downstream","node_id":peer["node_id"],"url":null,"remote_address":peer["remote_address"],"status":if disabled {"disconnected"}else if online {"connected"}else{"offline"},"disabled":disabled}));
        }
        Ok(json!(rows))
    }
    pub(crate) async fn configure_connection(
        &self,
        project: Uuid,
        input: Value,
    ) -> Result<Value, String> {
        let _guard = self.lifecycle_lock().lock().await;
        let rows = self.connections(project).await?;
        let row = rows
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == input["name"] && r["direction"] == input["direction"])
            .ok_or("unknown connection in this project")?;
        let deleted = input["deleted"] == true;
        let disabled = deleted
            || input["disabled"]
                .as_bool()
                .ok_or("disabled must be boolean")?;
        let name = row["name"].as_str().unwrap();
        let downstream = row["direction"] == "downstream";
        if !disabled
            && row["disabled"] == false
            && matches!(row["status"].as_str(), Some("connected" | "connecting"))
        {
            return Ok(row.clone());
        }
        let key = if downstream {
            format!("{project}:{name}")
        } else {
            name.into()
        };
        self.state().store.transaction(|data|{
            let collection=if downstream {"peer_blocks"}else{"peer_link_settings"};
            let revision=data.get(collection,&key).and_then(|r|r["revision"].as_u64()).unwrap_or(0)+1;
            data.set(collection,&key,json!({"disabled":disabled,"deleted":deleted,"revision":revision,"last_status":if disabled {"disconnected"}else{"connecting"}}));
            Ok(())
        }).await?;
        if downstream && disabled {
            self.state()
                .clients
                .lock()
                .await
                .remove(&(project, name.into()));
        }
        if !downstream {
            self.state().link_status.lock().await.insert(
                name.into(),
                if disabled {
                    "disconnected"
                } else {
                    "connecting"
                }
                .into(),
            );
        }
        Ok(json!({"name":name,"disabled":disabled,"deleted":deleted}))
    }
}
