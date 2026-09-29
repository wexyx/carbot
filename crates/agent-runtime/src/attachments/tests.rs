use super::*;
#[test]
fn files_are_referenced_not_embedded_and_paths_are_validated() {
    let dir = tempfile::tempdir().unwrap();
    let store = AttachmentStore::new(dir.path().join("attachments"));
    let file = store.upload("hello.txt", b"hello").unwrap();
    let ids = AttachmentStore::references(&file.reference());
    assert_eq!(ids.len(), 1);
    assert_eq!(store.read(&ids[0]).unwrap().1, b"hello");
    assert!(store.read("../../secret").is_err());
    assert!(store.upload("../bad", b"bad").is_err());
    assert!(store.register(dir.path()).is_err());
    let local = dir.path().join("local.txt");
    std::fs::write(&local, "local content").unwrap();
    let file = store.register(&local).unwrap();
    let ids = AttachmentStore::references(&file.reference());
    assert_eq!(
        store.read(&ids[0]).unwrap().2.unwrap(),
        local.canonicalize().unwrap()
    );
    let (prompt, prepared) =
        PreparedAttachments::prepare(&store, &file.reference(), dir.path()).unwrap();
    assert!(prompt.contains("local content"));
    drop(prepared);
    assert!(store.upload("large", &vec![0; MAX_FILE_BYTES + 1]).is_err());
}
#[tokio::test]
async fn image_materialization_is_scoped_and_never_placed_in_text() {
    let dir = tempfile::tempdir().unwrap();
    let store = AttachmentStore::new(dir.path().join("attachments"));
    let file = store
        .upload("photo.png", b"\x89PNG\r\n\x1a\nfixture")
        .unwrap();
    let (text, prepared) =
        PreparedAttachments::prepare(&store, &file.reference(), dir.path()).unwrap();
    assert!(!text.contains("base64"));
    assert!(!text.contains("fixture"));
    prepared
        .scope(async {
            PreparedAttachments::images(|images| {
                assert_eq!(images.len(), 1);
                assert_eq!(images[0].media_type, "image/png");
                assert!(images[0].path.is_file());
                assert!(
                    images[0]
                        .path
                        .starts_with(crate::workspace::temporary_dir(dir.path()).unwrap())
                );
            });
        })
        .await;
    assert_eq!(PreparedAttachments::images(|v| v.len()), 0);
    assert_eq!(
        std::fs::read_dir(crate::workspace::temporary_dir(dir.path()).unwrap())
            .unwrap()
            .count(),
        0
    );
    assert!(
        store
            .read(&AttachmentStore::references(&file.reference())[0])
            .is_ok()
    );
}
