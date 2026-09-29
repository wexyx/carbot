use super::BrowserExecution;
use crate::permissions::PermissionMode;

#[tokio::test]
async fn missing_runtime_requires_approval_before_creating_dependencies() {
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let runtime = super::runtime::BrowserRuntime::new(data.path().into());
    let correlation = uuid::Uuid::new_v4();
    let outcome = crate::workspace::with_approval_context(correlation, async {
        tokio::join!(
            PermissionMode::Ask.scope(runtime.ensure(work.path())),
            async {
                tokio::time::timeout(std::time::Duration::from_secs(10), async {
                    loop {
                        if let Some(request) = crate::workspace::pending()
                            .into_iter()
                            .find(|r| r.correlation_id == Some(correlation))
                        {
                            assert!(
                                request
                                    .command
                                    .as_deref()
                                    .unwrap()
                                    .contains("安装浏览器 Skill 依赖")
                            );
                            crate::workspace::decide(request.id, false).unwrap();
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                    }
                })
                .await
                .unwrap();
            }
        )
        .0
    })
    .await;
    assert!(outcome.is_err());
    assert!(!data.path().join("runtime").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn native_runtime_reuses_the_instance_directory_without_installing() {
    use std::os::unix::fs::PermissionsExt;
    let data = tempfile::tempdir().unwrap();
    let work = tempfile::tempdir().unwrap();
    let runtime = data.path().join("runtime/browser-automation/.runtime");
    let package = runtime.join("node_modules/puppeteer");
    std::fs::create_dir_all(&package).unwrap();
    std::fs::create_dir_all(runtime.join("browsers")).unwrap();
    std::fs::write(
        package.join("package.json"),
        r#"{"version":"25.12.0","main":"index.cjs"}"#,
    )
    .unwrap();
    std::fs::write(
        package.join("index.cjs"),
        "exports.executablePath=()=>require('path').join(process.env.PUPPETEER_CACHE_DIR,'shell')",
    )
    .unwrap();
    let binary = runtime.join("browsers/shell");
    std::fs::write(&binary, "fixture").unwrap();
    std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o700)).unwrap();
    let resolved = super::runtime::BrowserRuntime::new(data.path().into())
        .ensure(work.path())
        .await
        .unwrap();
    assert_eq!(resolved, runtime);
    assert!(!work.path().join("runtime").exists());
}

#[tokio::test]
async fn rejects_non_web_urls_before_any_launch() {
    let dir = tempfile::tempdir().unwrap();
    for url in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "https://user:secret@example.com",
    ] {
        assert!(BrowserExecution::run(dir.path(), url, false).await.is_err());
    }
}

#[tokio::test]
async fn host_browser_renders_and_screenshots_with_native_browser_sandbox() {
    if !std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../skills/system/business/browser-automation/.runtime/node_modules/puppeteer")
        .exists()
    {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                break;
            };
            tokio::spawn(async move {
                use tokio::io::{AsyncReadExt, AsyncWriteExt};
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).await;
                let body = "<title>Browser test</title><h1>browser-host-ok</h1>";
                let _=stream.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await;
            });
        }
    });
    let result = PermissionMode::Full
        .scope(BrowserExecution::execute(
            root.path(),
            &url,
            true,
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../skills/system/business/browser-automation/.runtime"),
        ))
        .await;
    server.abort();
    let result = result.unwrap();
    assert_eq!(result["title"], "Browser test");
    assert!(result["text"].as_str().unwrap().contains("browser-host-ok"));
    let screenshot = std::path::Path::new(result["screenshot"].as_str().unwrap());
    assert!(screenshot.starts_with(crate::workspace::temporary_dir(root.path()).unwrap()));
    assert!(std::fs::read(screenshot).unwrap().starts_with(b"\x89PNG"));
    assert!(!include_str!("bridge.mjs").contains("--no-sandbox"));
}
