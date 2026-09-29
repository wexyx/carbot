use super::NativeCommand;
use std::path::Path;
use tokio::io::AsyncWriteExt;

async fn endpoint() -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                use tokio::io::AsyncReadExt;
                let mut input = [0; 2048];
                let _ = socket.read(&mut input).await;
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                    )
                    .await;
            });
        }
    });
    (url, task)
}

#[tokio::test]
async fn python_and_shell_use_host_network_and_filesystem() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "host file").unwrap();
    let (url, server) = endpoint().await;
    let native = NativeCommand::new(root.path()).unwrap();
    let mut python = native.command(Path::new("python3"), root.path()).unwrap();
    python.args(["-I","-c",r#"import pathlib,sys,urllib.request
pathlib.Path('python-proof').write_text('real file')
assert pathlib.Path(sys.argv[1]).read_text() == 'host file'
pathlib.Path(sys.argv[1]).write_text('updated')
print(urllib.request.build_opener(urllib.request.ProxyHandler({})).open(sys.argv[2],timeout=4).read().decode())
"#]).arg(outside.path().join("secret")).arg(&url);
    let py = tokio::time::timeout(std::time::Duration::from_secs(8), python.output()).await;
    let mut shell = native.command(Path::new("/bin/sh"), root.path()).unwrap();
    shell.args(["-c",&format!("printf real > shell-proof; test \"$(cat '{}')\" = updated || exit 9; curl --noproxy '*' --max-time 4 -fsS '{}'",outside.path().join("secret").display(),url)]);
    let sh = tokio::time::timeout(std::time::Duration::from_secs(8), shell.output()).await;
    server.abort();
    for output in [py.unwrap().unwrap(), sh.unwrap().unwrap()] {
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("ok"));
    }
    assert!(root.path().join("python-proof").exists());
    assert!(root.path().join("shell-proof").exists());
}
