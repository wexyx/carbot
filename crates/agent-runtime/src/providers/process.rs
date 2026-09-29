use tokio::io::{AsyncReadExt, BufReader};

pub(super) async fn capture_stderr(stderr: Option<tokio::process::ChildStderr>) -> String {
    let Some(stderr) = stderr else {
        return String::new();
    };
    let mut reader = BufReader::new(stderr);
    let mut output = String::new();
    let _ = reader.read_to_string(&mut output).await;
    output
}
