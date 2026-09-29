use serde_json::Value;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

/// Read newest records backwards without parsing old hours or file prefixes.
pub(super) fn read(paths: Vec<PathBuf>, before: u64, limit: usize) -> Result<Vec<Value>, String> {
    let mut result = Vec::new();
    for path in paths.into_iter().rev() {
        let mut file = File::open(&path).map_err(|e| e.to_string())?;
        let length = file.metadata().map_err(|e| e.to_string())?.len();
        if length == 0 {
            continue;
        }
        file.seek(SeekFrom::End(-1)).map_err(|e| e.to_string())?;
        let mut last = [0];
        file.read_exact(&mut last).map_err(|e| e.to_string())?;
        if last[0] != b'\n' {
            return Err(format!("incomplete chat log tail: {}", path.display()));
        }
        let mut offset = length - 1;
        let mut pending = Vec::new();
        loop {
            let count = offset.min(65536) as usize;
            offset -= count as u64;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            let mut bytes = vec![0; count];
            file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
            bytes.extend_from_slice(&pending);
            pending = bytes;
            while let Some(index) = pending.iter().rposition(|b| *b == b'\n') {
                let line = pending.split_off(index + 1);
                pending.truncate(index);
                push(&line, before, &mut result)?;
                if result.len() >= limit {
                    result.reverse();
                    return Ok(result);
                }
            }
            if pending.len() > 16 * 1024 * 1024 {
                return Err("JSONL record exceeds 16 MiB".into());
            }
            if offset == 0 {
                if !pending.is_empty() {
                    push(&pending, before, &mut result)?;
                }
                if result.len() >= limit {
                    result.reverse();
                    return Ok(result);
                }
                break;
            }
        }
    }
    result.reverse();
    Ok(result)
}
fn push(line: &[u8], before: u64, result: &mut Vec<Value>) -> Result<(), String> {
    if line.len() > 16 * 1024 * 1024 {
        return Err("JSONL record exceeds 16 MiB".into());
    }
    let row: Value = serde_json::from_slice(line).map_err(|e| format!("invalid chat log: {e}"))?;
    let seq = row["seq"].as_u64().ok_or("chat event missing seq")?;
    if seq > 0 && seq < before {
        result.push(row);
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn reads_tail_without_scanning_old_prefix_and_handles_long_unicode_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log");
        let first = json!({"seq":1,"content":"中文".repeat(30000)});
        std::fs::write(&path, format!("{first}\n{{\"seq\":2}}\n{{\"seq\":3}}\n")).unwrap();
        assert_eq!(
            read(vec![path.clone()], 3, 1).unwrap(),
            vec![json!({"seq":2})]
        );
        assert_eq!(read(vec![path.clone()], u64::MAX, 3).unwrap()[0], first);
        std::fs::write(&path, "unparsed old prefix\n{\"seq\":4}\n").unwrap();
        assert_eq!(
            read(vec![path.clone()], u64::MAX, 1).unwrap(),
            vec![json!({"seq":4})]
        );
        std::fs::write(&path, "{\"seq\":5}").unwrap();
        assert!(read(vec![path], u64::MAX, 1).is_err());
    }
}
