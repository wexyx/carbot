use crate::{AppState, node::Link};
use serde_json::{Value, json};
use std::time::Duration;

pub(super) async fn supervise(state: AppState, link: Link) {
    loop {
        let row = settings(&state, &link).await;
        if row["disabled"] == true || row["deleted"] == true {
            tokio::time::sleep(Duration::from_millis(250)).await;
            continue;
        }
        let revision = row["revision"].as_u64().unwrap_or(0);
        tokio::select! {
            _=super::node::run_link(state.clone(),link.clone()) => {
                let _=state.store.transaction(|data|{
                    let mut row=data.get("peer_link_settings",&link.name).cloned().unwrap_or(json!({}));
                    if row["revision"].as_u64().unwrap_or(0)==revision {
                        row["disabled"]=json!(true);row["last_status"]=json!("failed");
                        data.set("peer_link_settings",&link.name,row);
                    }
                    Ok(())
                }).await;
            },
            _=changed(&state,&link,revision)=>{},
        }
    }
}
async fn settings(state: &AppState, link: &Link) -> Value {
    state
        .store
        .get("peer_link_settings", &link.name)
        .await
        .unwrap_or(json!({}))
}
async fn changed(state: &AppState, link: &Link, revision: u64) {
    loop {
        let row = settings(state, link).await;
        if row["disabled"] == true
            || row["deleted"] == true
            || row["revision"].as_u64().unwrap_or(0) != revision
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
