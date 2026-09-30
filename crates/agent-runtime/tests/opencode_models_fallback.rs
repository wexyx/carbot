//! Exercises the degraded path: a CLI that only answers `opencode models` must
//! still produce a usable picker, with prices explicitly unknown rather than free.
use agent_runtime::config::{OpenCodeCatalogSource, OpenCodeConfig, opencode_models};
use std::os::unix::fs::PermissionsExt;

#[tokio::test]
async fn an_identifier_only_cli_still_yields_models_without_claiming_free() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("opencode");
    std::fs::write(
        &binary,
        r#"#!/bin/sh
case "$*" in
  *"api get /api/model"*) echo "unavailable" >&2; exit 1 ;;
  models) printf 'opencode/alpha\nopencode/beta-free\n' ;;
  *) exit 2 ;;
esac
"#,
    )
    .unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

    let mut config = OpenCodeConfig::from_env();
    config.binary = binary;
    let catalog = opencode_models(&config).await.expect("identifier fallback");
    assert_eq!(catalog.source, OpenCodeCatalogSource::Identifiers);
    assert_eq!(catalog.models.len(), 2);
    assert_eq!(catalog.models[0].id, "opencode/alpha");
    assert_eq!(catalog.models[1].name, "beta-free");
    for model in &catalog.models {
        // A `-free` suffix is not evidence; only a published zero price is.
        assert!(!model.free, "{} was called free without a price", model.id);
        assert!(!model.priced);
        assert_eq!(model.input, None);
    }
}

#[tokio::test]
async fn a_missing_cli_reports_both_attempts_instead_of_an_empty_picker() {
    let mut config = OpenCodeConfig::from_env();
    config.binary = "/nonexistent/opencode".into();
    let error = opencode_models(&config).await.unwrap_err();
    assert!(error.contains("无法执行"), "{error}");
}
