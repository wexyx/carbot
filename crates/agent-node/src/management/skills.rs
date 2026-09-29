use crate::core::Core;
use agent_runtime::skills::{SkillCatalog, SkillDefinition};
use serde_json::{Value, json};
use uuid::Uuid;
pub(super) async fn catalog(core: &Core, project: Uuid) -> Result<SkillCatalog, String> {
    crate::capabilities::Library::new(core.state().store.clone())
        .skills(
            "management",
            &crate::capabilities::Context::new(project, "admin"),
        )
        .await
}
pub(crate) async fn save(core: &Core, project: Uuid, input: Value) -> Result<Value, String> {
    core.project(project).await?;
    let definition: SkillDefinition =
        serde_json::from_value(input["definition"].clone()).map_err(|e| e.to_string())?;
    definition.validate()?;
    if definition.allow_python() || definition.id() == "management-guide" {
        return Err("management skills cannot enable Python or replace the built-in guide".into());
    }
    let version = input["expected_version"]
        .as_u64()
        .ok_or("expected_version required")?;
    let key = format!("{project}:{}", definition.id());
    core.state().store.transaction(|data| {
        let old=data.get("management_skills",&key).and_then(|r|r["version"].as_u64()).unwrap_or(0);
        if old!=version { return Err("version_conflict".into()); }
        let row=json!({"project_id":project,"version":version.checked_add(1).ok_or("version overflow")?,"definition":definition});
        data.set("management_skills",&key,row.clone());
        let definitions=data.list("management_skills").into_iter().filter(|r|r["project_id"]==json!(project)).map(|r|serde_json::from_value(r["definition"].clone()).map_err(|e|e.to_string())).collect::<Result<Vec<_>,_>>()?;
        if definitions.len()>31 {return Err("management package limit: 31 custom skills plus built-in guide".into());}
        SkillCatalog::new(definitions)?;
        Ok(row)
    }).await
}
