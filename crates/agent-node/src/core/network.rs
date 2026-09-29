use super::Core;
use serde_json::{Value, json};
use uuid::Uuid;
impl Core {
    pub(crate) async fn mount(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let _guard = self.lifecycle_lock().lock().await;
        if let Some(row) = self.existing_connection(project, &input).await? {
            return Ok(row);
        }
        crate::node::mount(self.state(), project, input).await
    }
    pub(crate) async fn create_project(&self, input: Value) -> Result<Value, String> {
        let name = input["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 255)
            .ok_or("name required")?;
        let space_name = input["space_name"]
            .as_str()
            .filter(|s| !s.trim().is_empty() && s.len() <= 255)
            .ok_or("space_name required")?;
        let space = Uuid::new_v4();
        let project = Uuid::new_v4();
        self.state().store.transaction(|d|{
            d.insert("spaces",&space.to_string(),json!({"id":space,"name":space_name,"created_at":crate::storage::now()}))?;
            d.insert("projects",&project.to_string(),json!({"id":project,"space_id":space,"name":name,"created_at":crate::storage::now()}))
        }).await?;
        Ok(
            json!({"project_id":project,"space_id":space,"name":name,"note":"Select the new project to start a separately scoped AdminAgent conversation."}),
        )
    }
}
