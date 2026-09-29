use crate::core::Core;
use uuid::Uuid;
pub(super) struct Context {
    core: Core,
    project: Uuid,
    manager: std::sync::Weak<super::Manager>,
}
impl Context {
    pub(super) fn new(core: Core, project: Uuid, manager: std::sync::Weak<super::Manager>) -> Self {
        Self {
            core,
            project,
            manager,
        }
    }
    pub(super) fn core(&self) -> &Core {
        &self.core
    }
    pub(super) fn project(&self) -> Uuid {
        self.project
    }
    pub(super) fn manager(&self) -> Result<std::sync::Arc<super::Manager>, String> {
        self.manager.upgrade().ok_or("AdminAgent stopped".into())
    }
}
