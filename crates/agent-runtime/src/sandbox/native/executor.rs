use super::{NativeCommand, ProcessGroup};
use crate::{
    sandbox::{
        executor::{ExecutionFuture, SandboxExecutor},
        profile::Profile,
    },
    skills::ExecutionRequest,
    workspace::{OutsideAccess, Workspace},
};
use serde_json::json;
use std::path::Path;
use tokio::io::AsyncReadExt;

pub(crate) struct NativeExecutor;
impl SandboxExecutor for NativeExecutor {
    fn execute<'a>(
        &'a self,
        request: &'a ExecutionRequest,
        profile: &'a Profile,
    ) -> ExecutionFuture<'a> {
        Box::pin(async move {
            request.validate()?;
            if !matches!(profile.network.as_str(), "none" | "host") {
                return Err("native network must be none or host".into());
            }
            let workspace = Workspace::new(request.workdir.clone(), OutsideAccess::from_env()?)?;
            let mut reads = Vec::new();
            let mut writes = Vec::new();
            for access in &request.access {
                let path = workspace
                    .authorize(
                        Path::new(&access.path),
                        if access.write {
                            "python read/write subtree"
                        } else {
                            "python read-only subtree"
                        },
                    )
                    .await?;
                if access.write {
                    writes.push(path);
                } else {
                    reads.push(path);
                }
            }
            let sandbox = NativeCommand::new()?;
            let package = sandbox.scratch().join("skill");
            for (name, content) in request.skill.files() {
                let file = package.join(name);
                std::fs::create_dir_all(file.parent().ok_or("invalid skill path")?)
                    .map_err(|e| e.to_string())?;
                std::fs::write(file, content).map_err(|e| e.to_string())?;
            }
            let binary = std::env::var("AGENT_PYTHON_BIN").unwrap_or_else(|_| "python3".into());
            let mut command =
                sandbox.command(Path::new(&binary), workspace.root(), &reads, &writes, true)?;
            command.args(["-I","-B","-c","import runpy,sys; p=sys.argv.pop(1); script=sys.argv.pop(1); sys.argv[0]=script; sys.path.insert(0,p); runpy.run_path(script,run_name='__main__')"])
 .arg(&package).arg(package.join(&request.path)).args(&request.args);
            for (key, value) in profile.secrets()? {
                command.env(key, value);
            }
            #[cfg(unix)]
            unsafe {
                let seconds = profile.timeout_seconds;
                command.pre_exec(move || {
                    let limit = libc::rlimit {
                        rlim_cur: seconds as libc::rlim_t,
                        rlim_max: seconds as libc::rlim_t,
                    };
                    if libc::setrlimit(libc::RLIMIT_CPU, &limit) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    let files = libc::rlimit {
                        rlim_cur: 16 * 1024 * 1024,
                        rlim_max: 16 * 1024 * 1024,
                    };
                    if libc::setrlimit(libc::RLIMIT_FSIZE, &files) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let mut child = command.spawn().map_err(|e| {
                format!("native sandbox launch failed; no direct execution fallback: {e}")
            })?;
            let _group = ProcessGroup::new(child.id().ok_or("missing process ID")?);
            let stdout = child.stdout.take().ok_or("missing stdout")?;
            let stderr = child.stderr.take().ok_or("missing stderr")?;
            let (status, stdout, stderr) = tokio::try_join!(
                async { child.wait().await.map_err(|e| e.to_string()) },
                read(stdout),
                read(stderr)
            )?;
            Ok(
                json!({"sandbox":if crate::permissions::PermissionMode::current()==crate::permissions::PermissionMode::Full {"disabled_explicit_full_access"} else {"native"},"profile":profile.id,"network":"host","workdir":workspace.root(),"exit_code":status.code(),"success":status.success(),"stdout":stdout,"stderr":stderr}),
            )
        })
    }
}
async fn read(reader: impl tokio::io::AsyncRead + Unpin) -> Result<String, String> {
    let mut bytes = Vec::new();
    reader
        .take(65537)
        .read_to_end(&mut bytes)
        .await
        .map_err(|e| e.to_string())?;
    if bytes.len() > 65536 {
        return Err("sandbox output exceeds 64 KiB".into());
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
