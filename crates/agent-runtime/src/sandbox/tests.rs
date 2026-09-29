use super::{
    executor::{ExecutionFuture, SandboxExecutor},
    profile::Profile,
    service::Sandbox,
};
use crate::skills::{ExecutionRequest, SkillDefinition};
use serde_json::json;
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
struct Fixture {
    entered: tokio::sync::Notify,
    dropped: Arc<AtomicUsize>,
    hang: bool,
}
struct Guard(Arc<AtomicUsize>);
impl Drop for Guard {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}
impl SandboxExecutor for Fixture {
    fn execute<'a>(
        &'a self,
        _request: &'a ExecutionRequest,
        _profile: &'a Profile,
    ) -> ExecutionFuture<'a> {
        Box::pin(async move {
            let _guard = Guard(self.dropped.clone());
            self.entered.notify_one();
            if self.hang {
                std::future::pending::<()>().await;
            }
            Ok(json!({"stdout":"ok"}))
        })
    }
}
fn setup(hang: bool) -> (Arc<Sandbox>, Arc<Fixture>, ExecutionRequest) {
    let fixture = Arc::new(Fixture {
        entered: tokio::sync::Notify::new(),
        dropped: Arc::new(AtomicUsize::new(0)),
        hang,
    });
    let profile =
        serde_json::from_value(json!({"id":"default","network":"host","timeout_seconds":1}))
            .unwrap();
    let sandbox = Arc::new(Sandbox::new(fixture.clone(), vec![profile]).unwrap());
    let skill = SkillDefinition::new(
        "test".into(),
        "test".into(),
        BTreeMap::from([
            ("SKILL.md".into(), "test".into()),
            ("scripts/run.py".into(), "print('ok')".into()),
        ]),
        true,
        true,
    )
    .unwrap();
    let request = ExecutionRequest {
        access: vec![],
        workdir: ".".into(),
        skill,
        path: "scripts/run.py".into(),
        args: vec![],
        profile: "default".into(),
    };
    (sandbox, fixture, request)
}
#[tokio::test]
async fn native_supervisor_cancellation_releases_execution_before_shutdown() {
    let (sandbox, fixture, request) = setup(true);
    let task = {
        let s = sandbox.clone();
        tokio::spawn(async move { s.execute(request).await })
    };
    tokio::time::timeout(Duration::from_secs(2), fixture.entered.notified())
        .await
        .unwrap();
    task.abort();
    sandbox.shutdown().await;
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn native_supervisor_timeout_and_shutdown_fail_closed() {
    let (sandbox, fixture, request) = setup(true);
    assert!(
        sandbox
            .execute(request.clone())
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    sandbox.shutdown().await;
    assert!(sandbox.execute(request).await.is_err());
}
#[tokio::test]
async fn native_supervisor_success_and_unknown_profile() {
    let (sandbox, fixture, request) = setup(false);
    assert_eq!(
        sandbox.execute(request.clone()).await.unwrap()["stdout"],
        "ok"
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    let mut invalid = request;
    invalid.profile = "unknown".into();
    assert!(sandbox.execute(invalid).await.is_err());
}
#[test]
fn native_profile_rejects_retired_fields_and_unsafe_environment() {
    let base = json!({"id":"default","network":"host","timeout_seconds":30});
    let profile: Profile = serde_json::from_value(base.clone()).unwrap();
    profile.validate().unwrap();
    for field in [
        "image",
        "runtime",
        "cpus",
        "memory_mb",
        "pids",
        "scratch_mb",
    ] {
        let mut input = base.clone();
        input[field] = json!("retired");
        assert!(serde_json::from_value::<Profile>(input).is_err());
    }
    for name in [
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
        "PYTHONPATH",
        "PATH",
        "HOME",
    ] {
        let mut input = base.clone();
        input["secret_env"] = json!([name]);
        assert!(
            serde_json::from_value::<Profile>(input)
                .unwrap()
                .validate()
                .is_err()
        );
    }
}
#[cfg(target_os = "macos")]
#[tokio::test]
async fn native_directory_boundary_denies_sibling_files_and_allows_workdir() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("work");
    std::fs::create_dir(&root).unwrap();
    let secret = dir.path().join("outside.txt");
    std::fs::write(&secret, "private").unwrap();
    let forbidden = dir.path().join("forbidden.txt");
    let sandbox = super::native::NativeCommand::new().unwrap();
    let mut command = sandbox
        .command(std::path::Path::new("/bin/sh"), &root, &[], &[], false)
        .unwrap();
    command.args(["-c","printf inside > inside.txt; if cat \"$1\" >/dev/null 2>&1; then exit 12; fi; if (printf bad > \"$2\") 2>/dev/null; then exit 13; fi; exit 0","fixture"]).arg(&secret).arg(&forbidden);
    let output = tokio::time::timeout(Duration::from_secs(5), command.output())
        .await
        .unwrap()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        std::fs::read_to_string(root.join("inside.txt")).unwrap(),
        "inside"
    );
    assert!(!forbidden.exists());
}
