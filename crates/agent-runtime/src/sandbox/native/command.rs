use std::{
    path::{Path, PathBuf},
    process::Stdio,
};
use tokio::process::Command;

/// Platform isolation, separate from cwd and approval policy. Never falls back to direct execution.
pub(crate) struct NativeCommand {
    scratch: tempfile::TempDir,
}
impl NativeCommand {
    pub(crate) async fn import_credential(
        &self,
        root: &Path,
        source: &Path,
        destination: &Path,
    ) -> Result<(), String> {
        if !source.is_file() {
            return Ok(());
        }
        let workspace = crate::workspace::Workspace::new(
            root.into(),
            crate::workspace::OutsideAccess::from_env()?,
        )?;
        let approved = workspace
            .authorize(
                source,
                "provider credential read (copy into isolated runtime)",
            )
            .await?;
        let target = self.scratch.path().join(destination);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(approved, target).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub(crate) fn new() -> Result<Self, String> {
        Ok(Self {
            scratch: tempfile::Builder::new()
                .prefix("agent-sandbox-")
                .tempdir()
                .map_err(|e| e.to_string())?,
        })
    }
    pub(crate) fn scratch(&self) -> &Path {
        self.scratch.path()
    }
    pub(crate) fn command(
        &self,
        binary: &Path,
        root: &Path,
        reads: &[PathBuf],
        writes: &[PathBuf],
        network: bool,
    ) -> Result<Command, String> {
        let binary = resolve_binary(binary)?;
        let root = root.canonicalize().map_err(|e| e.to_string())?;
        if !root.is_dir() {
            return Err("workdir is not a directory".into());
        }
        let scratch = self
            .scratch
            .path()
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let mut read_paths = system_paths();
        read_paths.push(binary.clone());
        // Installed runtimes may live in versioned prefixes outside the system dirs.
        if let Some(parent) = binary.parent() {
            read_paths.push(parent.into());
        }
        read_paths.extend_from_slice(reads);
        let mut write_paths = vec![root.clone(), scratch.clone()];
        write_paths.extend_from_slice(writes);
        let mut command = if crate::permissions::PermissionMode::current()
            == crate::permissions::PermissionMode::Full
        {
            Command::new(&binary)
        } else {
            platform_command(&binary, &root, &read_paths, &write_paths, network)?
        };
        command
            .current_dir(root)
            .env_clear()
            .env("HOME", &scratch)
            .env("TMPDIR", &scratch)
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("LANG", "en_US.UTF-8")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        if network {
            for key in [
                "HTTP_PROXY",
                "HTTPS_PROXY",
                "ALL_PROXY",
                "NO_PROXY",
                "http_proxy",
                "https_proxy",
                "all_proxy",
                "no_proxy",
            ] {
                if let Some(value) = std::env::var_os(key) {
                    command.env(key, value);
                }
            }
        }
        #[cfg(unix)]
        command.process_group(0);
        Ok(command)
    }
}
fn resolve_binary(binary: &Path) -> Result<PathBuf, String> {
    let resolved = if binary.components().count() > 1 {
        binary.canonicalize().ok()
    } else {
        std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
            .find_map(|p| p.join(binary).canonicalize().ok())
    };
    resolved
        .filter(|p| p.is_file())
        .ok_or_else(|| format!("executable not found: {}", binary.display()))
}
fn system_paths() -> Vec<PathBuf> {
    [
        "/usr",
        "/bin",
        "/sbin",
        "/lib",
        "/lib64",
        "/System",
        "/Library/Apple",
        "/opt/homebrew",
        "/usr/local",
        "/private/etc/ssl",
        "/etc/ssl",
        "/etc/resolv.conf",
        "/etc/hosts",
        "/etc/nsswitch.conf",
        "/etc/ld.so.cache",
    ]
    .into_iter()
    .map(PathBuf::from)
    .filter(|p| p.exists())
    .collect()
}
#[cfg(target_os = "macos")]
fn platform_command(
    binary: &Path,
    _root: &Path,
    reads: &[PathBuf],
    writes: &[PathBuf],
    network: bool,
) -> Result<Command, String> {
    if !Path::new("/usr/bin/sandbox-exec").is_file() {
        return Err("native sandbox unavailable: sandbox-exec is required".into());
    }
    let quote = |p: &Path| serde_json::to_string(&p.to_string_lossy()).unwrap();
    let mut profile = String::from(
        "(version 1)(deny default)(allow process-exec process-fork sysctl-read mach-lookup file-read-metadata)(allow file-read* file-write* (literal \"/dev/null\") (literal \"/dev/urandom\") (literal \"/dev/random\"))",
    );
    // macOS libSystem traverses ancestor directories during process bootstrap.
    // Grant only these exact directories, never their file contents/subtrees.
    let mut ancestors = std::collections::BTreeSet::new();
    for path in reads.iter().chain(writes) {
        for ancestor in path.ancestors().skip(1) {
            ancestors.insert(ancestor.to_path_buf());
        }
    }
    for path in ancestors {
        profile.push_str(&format!(
            "(allow file-read* (require-all (literal {}) (vnode-type DIRECTORY)))",
            quote(&path)
        ));
    }
    for path in reads {
        profile.push_str(&format!("(allow file-read* (subpath {}))", quote(path)));
    }
    for path in writes {
        profile.push_str(&format!(
            "(allow file-read* file-write* (subpath {}))",
            quote(path)
        ));
    }
    if network {
        profile.push_str("(allow network*)");
    }
    if let Ok(data) = std::fs::canonicalize(crate::paths::data_dir()) {
        profile.push_str(&format!(
            "(deny file-read* file-write* (subpath {}))",
            quote(&data)
        ));
    }
    let mut command = Command::new("/usr/bin/sandbox-exec");
    command.args(["-p", &profile]).arg(binary);
    Ok(command)
}
#[cfg(target_os = "linux")]
fn platform_command(
    binary: &Path,
    root: &Path,
    reads: &[PathBuf],
    writes: &[PathBuf],
    network: bool,
) -> Result<Command, String> {
    let bwrap = resolve_binary(Path::new("bwrap"))
        .map_err(|_| "native sandbox unavailable: install bubblewrap and enable user namespaces")?;
    let mut command = Command::new(bwrap);
    command.args([
        "--die-with-parent",
        "--new-session",
        "--unshare-user",
        "--unshare-pid",
        "--unshare-ipc",
        "--unshare-uts",
        "--cap-drop",
        "ALL",
        "--proc",
        "/proc",
        "--dev",
        "/dev",
        "--tmpfs",
        "/tmp",
    ]);
    if !network {
        command.arg("--unshare-net");
    }
    for path in reads {
        command.arg("--ro-bind").arg(path).arg(path);
    }
    for path in writes {
        command.arg("--bind").arg(path).arg(path);
    }
    if let Ok(data) = std::fs::canonicalize(crate::paths::data_dir()) {
        if writes.iter().any(|p| data.starts_with(p)) {
            command.arg("--tmpfs").arg(data);
        }
    }
    command.arg("--chdir").arg(root).arg("--").arg(binary);
    Ok(command)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn platform_command(
    _binary: &Path,
    _root: &Path,
    _reads: &[PathBuf],
    _writes: &[PathBuf],
    _network: bool,
) -> Result<Command, String> {
    Err("native directory sandbox currently supports macOS and Linux only".into())
}
