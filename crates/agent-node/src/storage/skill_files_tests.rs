use super::*;
use serde_json::{Value, json};

fn skill() -> Value {
    json!({"id":"demo","description":"test skill","enabled":true,"allow_python":false,
        "files":{"SKILL.md":"original","scripts/example.py":"print('hello')"}})
}

#[tokio::test]
async fn user_skill_files_are_instance_local_and_reloadable() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let store = open(a.path()).await.unwrap();
    let other = open(b.path()).await.unwrap();
    let row = json!({"id":"demo","kind":"skill","scope":"business","definition":skill(),"version":1,"readonly":false,"origin":null});
    store
        .insert("capability_library", "demo", row.clone())
        .await
        .unwrap();
    other
        .insert("capability_library", "demo", row)
        .await
        .unwrap();
    let stored = store.get("capability_library", "demo").await.unwrap();
    let relative = stored["skill_files_directory"].as_str().unwrap();
    assert!(relative.starts_with("skills/user/business/demo-"));
    let path = a.path().join(relative);
    assert_eq!(
        std::fs::read_to_string(path.join("scripts/example.py")).unwrap(),
        "print('hello')"
    );
    assert!(!a.path().join("skills/system").exists());
    let journal = std::fs::read_to_string(a.path().join("state.jsonl")).unwrap();
    let transaction: Value = serde_json::from_str(journal.trim()).unwrap();
    assert!(
        transaction["changes"][0]["value"]["definition"]
            .get("files")
            .is_none()
    );
    std::fs::write(path.join("SKILL.md"), "local edit").unwrap();
    let library = crate::capabilities::Library::new(store.clone());
    let resources = library.resources("business", "skill").await.unwrap();
    assert_eq!(
        resources
            .iter()
            .find(|r| r.id == "demo")
            .unwrap()
            .definition["files"]["SKILL.md"],
        "local edit"
    );
    let other_row = other.get("capability_library", "demo").await.unwrap();
    assert_eq!(
        other.skill_row("capability_library", &other_row).unwrap()["definition"]["files"]["SKILL.md"],
        "original"
    );
    drop(library);
    drop(store);
    let reopened = open(a.path()).await.unwrap();
    assert_eq!(
        reopened.get("capability_library", "demo").await.unwrap()["definition"]["files"]["SKILL.md"],
        "local edit"
    );
}

#[tokio::test]
async fn both_legacy_creation_paths_write_files_and_rejected_transactions_do_not_publish() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path()).await.unwrap();
    for (collection, field) in [("skills", "skill"), ("management_skills", "definition")] {
        store
            .insert(
                collection,
                "project:demo",
                json!({field:skill(),"version":1,"revision":1}),
            )
            .await
            .unwrap();
        let row = store.get(collection, "project:demo").await.unwrap();
        let path = dir
            .path()
            .join(row["skill_files_directory"].as_str().unwrap());
        assert!(path.join("SKILL.md").is_file());
    }
    let before = std::fs::read(dir.path().join("state.jsonl")).unwrap();
    let result: Result<(), String> = store
        .transaction(|data| {
            data.set(
                "skills",
                "project:demo",
                json!({"skill":skill(),"revision":2}),
            );
            Err("version_conflict".into())
        })
        .await;
    assert!(result.is_err());
    assert_eq!(
        before,
        std::fs::read(dir.path().join("state.jsonl")).unwrap()
    );
    assert_eq!(
        store.get("skills", "project:demo").await.unwrap()["revision"],
        1
    );
}

#[tokio::test]
async fn update_and_delete_keep_previous_content_recoverable() {
    let dir = tempfile::tempdir().unwrap();
    let store = open(dir.path()).await.unwrap();
    let library = crate::capabilities::Library::new(store.clone());
    let created = library
        .save(
            "business",
            "skill",
            json!({"expected_version":0,"definition":skill()}),
        )
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    let first = store.get("capability_library", id).await.unwrap();
    let mut edited = skill();
    edited["files"]["SKILL.md"] = json!("updated");
    edited["files"]
        .as_object_mut()
        .unwrap()
        .remove("scripts/example.py");
    library
        .save(
            "business",
            "skill",
            json!({"id":id,"expected_version":1,"definition":edited}),
        )
        .await
        .unwrap();
    let second = store.get("capability_library", id).await.unwrap();
    assert_ne!(
        first["skill_files_directory"],
        second["skill_files_directory"]
    );
    assert!(
        dir.path()
            .join(first["skill_files_directory"].as_str().unwrap())
            .join("scripts/example.py")
            .exists()
    );
    assert!(
        !dir.path()
            .join(second["skill_files_directory"].as_str().unwrap())
            .join("scripts/example.py")
            .exists()
    );
    library
        .save(
            "business",
            "skill",
            json!({"id":id,"expected_version":2,"deleted":true}),
        )
        .await
        .unwrap();
    assert!(
        !library
            .resources("business", "skill")
            .await
            .unwrap()
            .iter()
            .any(|r| r.id == id)
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_cannot_redirect_skill_writes_outside_instance() {
    let dir = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let store = open(dir.path()).await.unwrap();
    std::os::unix::fs::symlink(outside.path(), dir.path().join("skills")).unwrap();
    assert!(
        store
            .insert("skills", "p:demo", json!({"skill":skill(),"revision":1}))
            .await
            .is_err()
    );
    assert!(store.get("skills", "p:demo").await.is_none());
    assert_eq!(std::fs::read_dir(outside.path()).unwrap().count(), 0);
}
