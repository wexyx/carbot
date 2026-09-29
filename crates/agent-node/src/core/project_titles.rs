use crate::AppState;
use serde_json::json;
use uuid::Uuid;
pub(super) async fn from_message(
    state: &AppState,
    project: Uuid,
    group: &str,
    content: &str,
) -> Result<(), String> {
    if content.trim().is_empty() {
        return Ok(());
    }
    if state.policy_store.get(project, "group", group).await?.body["auto_name"] != true {
        return Ok(());
    }
    let text = content.split_whitespace().collect::<Vec<_>>().join(" ");
    let title: String = text.chars().take(32).collect();
    state
        .store
        .transaction(|data| {
            let collection = format!("documents:{project}:group");
            let mut row = data.get(&collection, group).cloned().ok_or("not_found")?;
            if row["body"]["auto_name"] != true {
                return Ok(());
            }
            row["body"]["name"] = json!(title);
            row["body"]["auto_name"] = json!(false);
            row["version"] = json!(
                row["version"]
                    .as_u64()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or("version overflow")?
            );
            data.set(&collection, group, row);
            Ok(())
        })
        .await
}
