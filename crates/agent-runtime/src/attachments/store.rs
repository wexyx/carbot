use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
};
use uuid::Uuid;

pub const MAX_FILE_BYTES: usize = 10 * 1024 * 1024;
const PREFIX: &str = "/v1/attachments/";

/// Public metadata only. File contents never enter the conversation journal.
#[derive(Clone, Serialize, Deserialize)]
pub struct Attachment {
    id: String,
    name: String,
    size: usize,
    media_type: String,
    url: String,
}
impl Attachment {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn media_type(&self) -> &str {
        &self.media_type
    }
    pub fn preview_url(&self) -> String {
        format!("{}?preview=true", self.url)
    }
    pub fn reference(&self) -> String {
        let name = self.name.replace(['[', ']', '\\', '\n', '\r'], "_");
        format!("[附件：{name}]({})", self.url)
    }
}

#[derive(Serialize, Deserialize)]
struct Record {
    attachment: Attachment,
    local: Option<PathBuf>,
}

/// Per-instance attachment snapshots. Model image publishing must first validate the tmp boundary.
pub struct AttachmentStore {
    root: PathBuf,
}
impl Default for AttachmentStore {
    fn default() -> Self {
        Self::new(crate::paths::data_dir().join("tmp/attachments"))
    }
}
impl AttachmentStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
    pub fn upload(&self, name: &str, bytes: &[u8]) -> Result<Attachment, String> {
        self.insert(name, bytes, None)
    }
    pub(crate) fn publish_image(&self, path: &Path) -> Result<Attachment, String> {
        let bytes = read_regular(path, 5 * 1024 * 1024)?;
        if !media_type(&bytes).starts_with("image/") {
            return Err("only PNG, JPEG, GIF and WebP images can be displayed".into());
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("invalid image filename")?;
        self.upload(name, &bytes)
    }
    pub fn register(&self, path: &Path) -> Result<Attachment, String> {
        let path = path.canonicalize().map_err(|e| e.to_string())?;
        let bytes = read_regular(&path, MAX_FILE_BYTES)?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or("文件名不是 UTF-8")?;
        self.insert(name, &bytes, Some(path.clone()))
    }
    fn insert(
        &self,
        name: &str,
        bytes: &[u8],
        local: Option<PathBuf>,
    ) -> Result<Attachment, String> {
        if name.is_empty()
            || name.len() > 240
            || name.chars().any(char::is_control)
            || name.contains(['/', '\\'])
        {
            return Err("文件名无效".into());
        }
        if bytes.len() > MAX_FILE_BYTES {
            return Err("附件不能超过 10 MiB".into());
        }
        let media_type = media_type(bytes);
        if media_type.starts_with("image/") && bytes.len() > 5 * 1024 * 1024 {
            return Err("图片不能超过 5 MiB".into());
        }
        std::fs::create_dir_all(&self.root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&self.root, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        // Publish an entire record atomically; interrupted uploads leave no usable reference.
        let staging = tempfile::Builder::new()
            .prefix("upload-")
            .tempdir_in(&self.root)
            .map_err(|e| e.to_string())?;
        let id = Uuid::new_v4().to_string();
        let attachment = Attachment {
            id: id.clone(),
            name: name.into(),
            size: bytes.len(),
            media_type: media_type.into(),
            url: format!("{PREFIX}{id}/content"),
        };
        if local.is_none() {
            private_write(&staging.path().join("content"), bytes)?;
        }
        let record = Record {
            attachment: attachment.clone(),
            local,
        };
        private_write(
            &staging.path().join("record.json"),
            &serde_json::to_vec(&record).map_err(|e| e.to_string())?,
        )?;
        std::fs::rename(staging.path(), self.root.join(&id)).map_err(|e| e.to_string())?;
        Ok(attachment)
    }
    pub fn read(&self, id: &str) -> Result<(Attachment, Vec<u8>, Option<PathBuf>), String> {
        let id = Uuid::parse_str(id).map_err(|_| "附件 ID 无效")?.to_string();
        let directory = self.root.join(&id);
        let record: Record =
            serde_json::from_slice(&read_regular(&directory.join("record.json"), 8192)?)
                .map_err(|_| "附件记录损坏")?;
        let uploaded = directory.join("content");
        let bytes = read_regular(record.local.as_deref().unwrap_or(&uploaded), MAX_FILE_BYTES)?;
        if record.attachment.id != id
            || bytes.len() != record.attachment.size
            || media_type(&bytes) != record.attachment.media_type
        {
            return Err("附件已变化，请重新添加".into());
        }
        Ok((record.attachment, bytes, record.local))
    }
    pub(crate) fn references(prompt: &str) -> Vec<String> {
        let mut ids = Vec::new();
        for tail in prompt.split(PREFIX).skip(1) {
            let Some(id) = tail
                .get(..36)
                .filter(|_| tail.get(36..).is_some_and(|s| s.starts_with("/content")))
            else {
                continue;
            };
            if Uuid::parse_str(id).is_ok() && !ids.iter().any(|v| v == id) {
                ids.push(id.to_owned());
            }
        }
        ids
    }
}
fn private_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| e.to_string())?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}
fn read_regular(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "附件不存在或无法读取，请重新添加")?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("只支持普通文件，不支持目录或设备".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > limit {
        return Err("附件超过大小限制".into());
    }
    Ok(bytes)
}
fn media_type(bytes: &[u8]) -> &'static str {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        "image/gif"
    } else if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        "image/webp"
    } else {
        "application/octet-stream"
    }
}
