use super::{
    executor::{ExecutionFuture, ProcessExecutor},
    profile::Profile,
    service::ExecutionService,
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
impl ProcessExecutor for Fixture {
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
fn setup(hang: bool) -> (Arc<ExecutionService>, Arc<Fixture>, ExecutionRequest) {
    let fixture = Arc::new(Fixture {
        entered: tokio::sync::Notify::new(),
        dropped: Arc::new(AtomicUsize::new(0)),
        hang,
    });
    let profile =
        serde_json::from_value(json!({"id":"default","network":"host","timeout_seconds":1}))
            .unwrap();
    let execution = Arc::new(ExecutionService::new(fixture.clone(), vec![profile]).unwrap());
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
    (execution, fixture, request)
}
#[tokio::test]
async fn native_supervisor_cancellation_releases_execution_before_shutdown() {
    let (execution, fixture, request) = setup(true);
    let task = {
        let s = execution.clone();
        tokio::spawn(async move { s.execute(request).await })
    };
    tokio::time::timeout(Duration::from_secs(2), fixture.entered.notified())
        .await
        .unwrap();
    task.abort();
    execution.shutdown().await;
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn native_supervisor_timeout_and_shutdown_fail_closed() {
    let (execution, fixture, request) = setup(true);
    assert!(
        execution
            .execute(request.clone())
            .await
            .unwrap_err()
            .contains("timed out")
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    execution.shutdown().await;
    assert!(execution.execute(request).await.is_err());
}
#[tokio::test]
async fn native_supervisor_success_and_unknown_profile() {
    let (execution, fixture, request) = setup(false);
    assert_eq!(
        execution.execute(request.clone()).await.unwrap()["stdout"],
        "ok"
    );
    assert_eq!(fixture.dropped.load(Ordering::SeqCst), 1);
    let mut invalid = request;
    invalid.profile = "unknown".into();
    assert!(execution.execute(invalid).await.is_err());
}
#[test]
fn native_profile_rejects_retired_fields_and_unsafe_environment() {
    let base = json!({"id":"default","network":"host","timeout_seconds":30});
    let profile: Profile = serde_json::from_value(base.clone()).unwrap();
    profile.validate().unwrap();
    let mut offline = profile.clone();
    offline.network = "none".into();
    assert!(
        offline
            .validate()
            .unwrap_err()
            .contains("no longer isolates")
    );
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

#[cfg(unix)]
#[tokio::test]
async fn cancellation_kills_spawned_children() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let task = tokio::spawn(async move {
        let profile =
            serde_json::from_value(json!({"id":"default","network":"host","timeout_seconds":10}))
                .unwrap();
        super::native::shell::execute(
            root,
            "(sleep 1; printf leaked > leaked) & printf ready > ready; wait".into(),
            profile,
        )
        .await
    });
    tokio::time::timeout(Duration::from_secs(5), async {
        while !dir.path().join("ready").exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    tokio::time::sleep(Duration::from_millis(1200)).await;
    assert!(!dir.path().join("leaked").exists());
}
