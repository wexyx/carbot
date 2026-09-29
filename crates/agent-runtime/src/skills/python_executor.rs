use super::{ExecutionPolicy, ExecutionRequest, SkillDefinition};
use serde_json::Value;
pub(crate) struct PythonExecutor {
    policy: ExecutionPolicy,
    workdir: std::path::PathBuf,
}
impl PythonExecutor {
    pub(crate) fn new(policy: ExecutionPolicy, workdir: std::path::PathBuf) -> Self {
        Self { policy, workdir }
    }
    pub(crate) async fn run(
        &self,
        skill: &SkillDefinition,
        path: &str,
        args: &[String],
        access: &[super::AccessRequest],
    ) -> Result<Value, String> {
        if !self.policy.python_enabled() {
            return Err("Python denied by host execution policy".into());
        }
        let request = ExecutionRequest {
            workdir: self.workdir.clone(),
            access: access.to_vec(),
            skill: skill.clone(),
            path: path.into(),
            args: args.to_vec(),
            profile: self.policy.select_profile(skill.execution_profile())?,
        };
        request.validate()?;
        crate::sandbox::execute(request).await
    }
}
