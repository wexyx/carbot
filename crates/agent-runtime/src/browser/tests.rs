use super::BrowserExecution;
use crate::permissions::PermissionMode;

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
        .scope(BrowserExecution::run(root.path(), &url, true))
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
