/// Killing the complete process group also covers script-spawned children.
pub(crate) struct ProcessGroup {
    pid: u32,
}
impl ProcessGroup {
    pub(crate) fn new(pid: u32) -> Self {
        Self { pid }
    }
}
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.pid as i32), libc::SIGKILL);
        }
    }
}
