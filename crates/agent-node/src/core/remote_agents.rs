use super::Core;
use crate::{AppState, control};
use futures_util::{StreamExt, stream};
use serde_json::{Value, json};
use uuid::Uuid;

impl Core {
    pub(crate) async fn agent_candidates(&self, p: Uuid) -> Result<Value, String> {
        let mut directory = self.agent_directory(p).await?;
        let original = directory["agents"].as_array().cloned().unwrap_or_default();
        let mut agents = vec![];
        let mut warnings = vec![];
        let mut remote = vec![];
        for mut row in original {
            let id = row["id"].as_str().unwrap_or_default().to_owned();
            if row["kind"] != "remote" {
                row["path"] = json!([id]);
                agents.push(row);
                continue;
            }
            remote.push((id, row));
        }
        let mut discovered = stream::iter(remote.into_iter().map(|(id, row)| async move {
            let result: Result<Vec<Value>, String> =
                Ok(self.remote_directory().snapshot(self.state(), p, &id).await);
            (id, row, result)
        }))
        .buffer_unordered(8);
        while let Some((id, row, result)) = discovered.next().await {
            match result {
                Ok(children) => {
                    for mut child in children {
                        let leaf = child["id"].as_str().unwrap_or_default().to_owned();
                        child["id"] = json!(format!("{id}/{leaf}"));
                        child["path"] = json!([id, leaf]);
                        child["node_id"] = row["node_id"].clone();
                        child["remote_address"] = row["remote_address"].clone();
                        child["node_name"] = row["role"].clone();
                        child["name"] = json!(format!(
                            "{} / {}",
                            row["role"].as_str().unwrap_or("远端"),
                            child["name"].as_str().unwrap_or(&leaf)
                        ));
                        child["agent_kind"] = child["kind"].clone();
                        child["kind"] = json!("remote");
                        agents.push(child);
                    }
                }
                Err(e) => warnings.push(json!({"node":id,"error":e})),
            }
        }
        agents.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
        directory["agents"] = json!(agents);
        directory["warnings"] = json!(warnings);
        Ok(directory)
    }
}
pub(super) async fn catalog(state: &AppState, p: Uuid, child: &str) -> Result<Vec<Value>, String> {
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        control::call(
            state,
            p,
            vec![child.into()],
            "agents.list".into(),
            json!({}),
            vec![],
        ),
    )
    .await
    .map_err(|_| "remote Agent discovery timed out")??;
    let rows = result.as_array().ok_or("invalid remote Agent catalog")?;
    if rows.len() > 256 {
        return Err("remote Agent catalog too large".into());
    }
    // Explicitly select public fields: no configuration secrets or remote mutation handles.
    rows.iter().map(|r|{
        let id=r["id"].as_str().ok_or("invalid remote Agent ID")?;
        control::validate_path(&[id.into()])?;
        Ok(json!({"id":id,"name":r["name"],"kind":r["kind"],"provider":r["provider"],"role":r["role"],"online":r["online"]}))
    }).collect()
}
