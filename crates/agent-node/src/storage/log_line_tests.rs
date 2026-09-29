use super::log_line_index::LineIndex;
use std::io::Write;
#[test]
fn indexed_cached_ranges_survive_append_and_invalidate_after_replacement() {
    let dir = tempfile::tempdir().unwrap();
    let name = "hour-000000000001.jsonl";
    let path = dir.path().join(name);
    let content = (1..=300)
        .map(|n| format!("{{\"n\":{n}}}\n"))
        .collect::<String>();
    std::fs::write(&path, content).unwrap();
    let mut index = LineIndex::default();
    let mut read =
        |from, to| index.read(LineIndex::open(dir.path(), name).unwrap(), name, from, to);
    let first = read(129, 132).unwrap();
    assert_eq!(first["lines"][0]["event"]["n"], 129);
    assert_eq!(first["lines"].as_array().unwrap().len(), 4);
    assert_eq!(read(129, 132).unwrap(), first);
    std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(b"{\"n\":301}\n")
        .unwrap();
    assert_eq!(read(301, 301).unwrap()["lines"][0]["event"]["n"], 301);
    assert!(read(0, 1).is_err());
    assert!(read(1, 201).is_err());
    std::fs::write(&path, "{\"n\":999}\n").unwrap();
    assert_eq!(read(1, 1).unwrap()["lines"][0]["event"]["n"], 999);
    assert!(LineIndex::open(dir.path(), "../secret.jsonl").is_err());
}
#[tokio::test]
async fn recall_is_bound_to_the_given_conversation() {
    let logs = super::ChatLog::temporary();
    let project = uuid::Uuid::new_v4();
    logs.append(
        project,
        "one".into(),
        vec![serde_json::json!({"content":"one"})],
    )
    .await
    .unwrap();
    logs.append(
        project,
        "two".into(),
        vec![serde_json::json!({"content":"two"})],
    )
    .await
    .unwrap();
    let index = logs
        .read_lines(project, "one".into(), None, 1, 1)
        .await
        .unwrap();
    let name = index[0]["file"].as_str().unwrap();
    let result = logs
        .read_lines(project, "one".into(), Some(name.into()), 1, 20)
        .await
        .unwrap();
    assert_eq!(result["lines"][0]["event"]["content"], "one");
    assert_eq!(
        logs.read_lines(uuid::Uuid::new_v4(), "one".into(), None, 1, 1)
            .await
            .unwrap(),
        serde_json::json!([])
    );
}
