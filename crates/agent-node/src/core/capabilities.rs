use super::Core;
use agent_runtime::skills::{SkillCatalog, SkillDefinition};
use serde_json::{Value, json};
use uuid::Uuid;
impl Core {
    pub(crate) async fn subtree_policy(
        &self,
        project: Uuid,
        operation: &str,
        mut input: Value,
    ) -> Result<Value, String> {
        self.project(project).await?;
        let path = serde_json::from_value(input["path"].take()).map_err(|e| e.to_string())?;
        crate::control::call(self.state(), project, path, operation.into(), input, vec![]).await
    }
    pub(crate) async fn save_skill(&self, project: Uuid, input: Value) -> Result<Value, String> {
        self.project(project).await?;
        let skill: SkillDefinition =
            serde_json::from_value(input["definition"].clone()).map_err(|e| e.to_string())?;
        skill.validate()?;
        if skill.allow_python() {
            return Err("AdminAgent cannot grant script execution permissions".into());
        }
        let revision = input["expected_version"]
            .as_u64()
            .ok_or("expected_version required")?;
        let key = format!("{project}:{}", skill.id());
        self.state().store.transaction(|d|{
            let old=d.get("skills",&key).and_then(|r|r["revision"].as_u64()).unwrap_or(0);
            if old!=revision{return Err("version_conflict".into());}
            let row=json!({"project_id":project,"revision":revision.checked_add(1).ok_or("version overflow")?,"skill":skill,"deleted":false});
            d.set("skills",&key,row.clone());
            let definitions=d.list("skills").into_iter().filter(|r|r["project_id"]==json!(project)&&r["deleted"]!=true).map(|r|serde_json::from_value(r["skill"].clone()).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
            SkillCatalog::new(definitions)?;
            Ok(row)
        }).await
    }
}
