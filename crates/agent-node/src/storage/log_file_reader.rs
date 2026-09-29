use serde_json::{Value, json};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};
pub(super) fn read(directory: &Path, name: &str, offset: u64) -> Result<Value, String> {
    if name.len() != 23
        || !name.starts_with("hour-")
        || !name.ends_with(".jsonl")
        || !name.as_bytes()[5..17].iter().all(|b| b.is_ascii_digit())
    {
        return Err("invalid log filename".into());
    }
    let dir = directory.canonicalize().map_err(|e| e.to_string())?;
    let path = dir.join(name).canonicalize().map_err(|e| e.to_string())?;
    if path.parent() != Some(dir.as_path()) {
        return Err("log file outside conversation".into());
    }
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let length = file.metadata().map_err(|e| e.to_string())?.len();
    if offset > length {
        return Err("offset outside log".into());
    }
    file.seek(SeekFrom::Start(offset))
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    file.take(128 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    // Preserve complete lines when possible, otherwise split huge events on a UTF-8 boundary.
    if offset + (bytes.len() as u64) < length {
        if let Some(end) = bytes.iter().rposition(|b| *b == b'\n') {
            bytes.truncate(end + 1);
        } else if let Err(error) = std::str::from_utf8(&bytes) {
            if error.error_len().is_none() {
                bytes.truncate(error.valid_up_to());
            }
        }
    }
    let next = offset + bytes.len() as u64;
    Ok(
        json!({"name":name,"offset":offset,"next_offset":next,"has_more":next<length,"content":String::from_utf8_lossy(&bytes)}),
    )
}
#[cfg(test)]
mod tests {
    #[test]
    fn rejects_paths_and_pages_content() {
        let dir = tempfile::tempdir().unwrap();
        let name = "hour-000000000001.jsonl";
        std::fs::write(dir.path().join(name), "{\"type\":\"user\"}\n".repeat(20000)).unwrap();
        assert!(super::read(dir.path(), "../secret", 0).is_err());
        let page = super::read(dir.path(), name, 0).unwrap();
        assert_eq!(page["has_more"], true);
        assert!(page["content"].as_str().unwrap().len() <= 128 * 1024);
        assert!(super::read(dir.path(), name, u64::MAX).is_err());
    }
}
