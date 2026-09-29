use super::*;
use serde_json::{Value, json};
use std::io::Write;

#[tokio::test]
async fn delta_writes_do_not_copy_unchanged_records_and_replay_deletions() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path()).await.unwrap();
    store
        .insert("large", "a", json!("x".repeat(1_000_000)))
        .await
        .unwrap();
    let path = dir.path().join("state.jsonl");
    let initial = std::fs::read(&path).unwrap();
    store.insert("small", "b", json!(1)).await.unwrap();
    let updated = std::fs::read(&path).unwrap();
    assert!(updated.starts_with(&initial));
    assert!(updated.len() - initial.len() < 200);
    let count = updated.len();
    store
        .transaction(|data| {
            data.set("small", "b", json!(1));
            Ok(())
        })
        .await
        .unwrap();
    assert_eq!(std::fs::metadata(&path).unwrap().len(), count as u64);
    let rejected: Result<(), String> = store
        .transaction(|data| {
            data.remove("large", "a");
            data.set("small", "b", json!(2));
            Err("reject".into())
        })
        .await;
    assert!(rejected.is_err());
    assert!(store.get("large", "a").await.is_some());
    assert_eq!(store.get("small", "b").await, Some(json!(1)));
    store
        .transaction(|data| {
            data.remove("large", "a");
            data.set("small", "b", Value::Null);
            Ok(())
        })
        .await
        .unwrap();
    drop(store);
    let store = open(dir.path()).await.unwrap();
    assert!(store.get("large", "a").await.is_none());
    assert_eq!(store.get("small", "b").await, Some(Value::Null));
}

#[tokio::test]
async fn legacy_snapshot_is_read_only_baseline_and_incomplete_tail_is_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let baseline = br#"{"format_version":1,"sequence":0,"collections":{"test":{"old":1}}}"#;
    std::fs::write(dir.path().join("state.json"), baseline).unwrap();
    let store = open(dir.path()).await.unwrap();
    store.insert("test", "new", json!(2)).await.unwrap();
    drop(store);
    assert_eq!(
        std::fs::read(dir.path().join("state.json")).unwrap(),
        baseline
    );
    let journal = dir.path().join("state.jsonl");
    let committed = std::fs::read(&journal).unwrap();
    std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"{\"partial\"")
        .unwrap();
    let store = open(dir.path()).await.unwrap();
    assert_eq!(store.get("test", "old").await, Some(json!(1)));
    assert_eq!(store.get("test", "new").await, Some(json!(2)));
    assert_eq!(std::fs::read(&journal).unwrap(), committed);
    drop(store);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&journal)
        .unwrap()
        .write_all(b"invalid\n")
        .unwrap();
    assert!(open(dir.path()).await.is_err());
}
