use super::Manager;
use serde_json::{Value, json};
use uuid::Uuid;

impl Manager {
    pub(crate) async fn list_skills(&self, project: Uuid, scope: &str) -> Result<Value, String> {
        self.core().project(project).await?;
        let collection = match scope {
            "management" => "management_skills",
            "business" => "skills",
            _ => return Err("invalid skill scope".into()),
        };
        let mut rows = vec![];
        for row in self.core().state().store.list(collection).await {
            if row["project_id"] != json!(project) || row["deleted"] == true {
                continue;
            }
            rows.push(if scope == "management" {
                json!({"definition":row["definition"],"version":row["version"],"readonly":false})
            } else {
                json!({"definition":row["skill"],"version":row["revision"],"readonly":false})
            });
        }
        if scope == "management" {
            rows.push(json!({"definition":{"id":"management-guide","description":"内置管理规则","enabled":true,"allow_python":false,"files":{"SKILL.md":include_str!("management-guide.md")}},"version":0,"readonly":true}));
        }
        rows.sort_by_key(|r| {
            r["definition"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        });
        Ok(json!(rows))
    }
    pub(crate) async fn save_skill(
        &self,
        project: Uuid,
        scope: &str,
        input: Value,
    ) -> Result<Value, String> {
        match scope {
            "management" => super::skills::save(self.core(), project, input).await,
            "business" => self.core().save_skill(project, input).await,
            _ => Err("invalid skill scope".into()),
        }
    }
}
