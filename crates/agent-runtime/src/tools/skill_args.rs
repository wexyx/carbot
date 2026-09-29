use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SkillArgs {
    #[serde(default)]
    pub(super) access: Vec<crate::skills::AccessRequest>,
    pub(super) skill_id: String,
    #[serde(default)]
    pub(super) path: String,
    #[serde(default)]
    pub(super) args: Vec<String>,
}
