use serde_json::{Value, json};
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};
const BLOCK: u64 = 128;
const MAX_LINE: u64 = 16 * 1024 * 1024;
const MAX_OUTPUT: usize = 128 * 1024;
/// Incremental sparse byte offsets; never deserialize the full journal to locate a range.
#[derive(Default)]
pub(super) struct LineIndex {
    cache: super::log_line_cache::LineCache,
    modified: Option<std::time::SystemTime>,
    identity: Option<(u64, u64)>,
    offsets: Vec<u64>,
    scanned: u64,
    lines: u64,
}
impl LineIndex {
    pub(super) fn open(directory: &Path, name: &str) -> Result<File, String> {
        if name.len() != 23
            || !name.starts_with("hour-")
            || !name.ends_with(".jsonl")
            || !name.as_bytes()[5..17].iter().all(u8::is_ascii_digit)
        {
            return Err("invalid log filename".into());
        }
        let root = directory.canonicalize().map_err(|e| e.to_string())?;
        let path = root.join(name).canonicalize().map_err(|e| e.to_string())?;
        if path.parent() != Some(root.as_path()) {
            return Err("log file outside conversation".into());
        }
        File::open(path).map_err(|e| e.to_string())
    }
    pub(super) fn refresh(&mut self, file: &mut File) -> Result<u64, String> {
        let metadata = file.metadata().map_err(|e| e.to_string())?;
        let length = metadata.len();
        #[cfg(unix)]
        let identity = {
            use std::os::unix::fs::MetadataExt;
            Some((metadata.dev(), metadata.ino()))
        };
        #[cfg(not(unix))]
        let identity = None;
        let modified = metadata.modified().ok();
        if length < self.scanned
            || self.identity.is_some_and(|old| Some(old) != identity)
            || (length == self.scanned && self.modified.is_some() && self.modified != modified)
        {
            *self = Self::default();
        }
        self.identity = identity;
        self.modified = modified;
        file.seek(SeekFrom::Start(self.scanned))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file.take(length - self.scanned));
        loop {
            let mut row = Vec::new();
            (&mut reader)
                .take(MAX_LINE + 1)
                .read_until(b'\n', &mut row)
                .map_err(|e| e.to_string())?;
            if row.len() as u64 > MAX_LINE {
                return Err("JSONL record exceeds 16 MiB".into());
            }
            if row.last() != Some(&b'\n') {
                break;
            }
            if self.lines % BLOCK == 0 {
                self.offsets.push(self.scanned);
            }
            self.scanned += row.len() as u64;
            self.lines += 1;
        }
        Ok(self.lines)
    }
    pub(super) fn read(
        &mut self,
        mut file: File,
        name: &str,
        from: u64,
        to: u64,
    ) -> Result<Value, String> {
        if from == 0 || to < from || to - from >= 200 {
            return Err("from_line/to_line must be 1-based inclusive, at most 200 lines".into());
        }
        self.refresh(&mut file)?;
        if from > self.lines {
            return Ok(json!({"file":name,"total_lines":self.lines,"lines":[],"has_more":false}));
        }
        let end = to.min(self.lines);
        let cached: Option<Vec<Value>> = (from..=end).map(|line| self.cache.get(line)).collect();
        if let Some(rows) = cached {
            let mut bytes = 0;
            let mut selected = Vec::new();
            for row in rows {
                let size = row.to_string().len();
                if bytes + size > MAX_OUTPUT && !selected.is_empty() {
                    break;
                }
                bytes += size;
                selected.push(row);
            }
            let next = from + selected.len() as u64;
            return Ok(
                json!({"file":name,"from_line":from,"to_line":next-1,"next_line":next,"total_lines":self.lines,"has_more":next<=self.lines,"lines":selected}),
            );
        }
        let block = (from - 1) / BLOCK;
        let offset = self.offsets[block as usize];
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(file.take(self.scanned - offset));
        let mut rows = Vec::new();
        let mut bytes = 0;
        let mut next = from;
        let mut position = offset;
        for line in block * BLOCK + 1..=to.min(self.lines) {
            let mut raw = Vec::new();
            reader
                .read_until(b'\n', &mut raw)
                .map_err(|e| e.to_string())?;
            let start = position;
            position += raw.len() as u64;
            if line < from {
                continue;
            }
            if bytes + raw.len() > MAX_OUTPUT && !rows.is_empty() {
                break;
            }
            if raw.len() > MAX_OUTPUT {
                rows.push(json!({"line":line,"truncated":true,"byte_offset":start,"byte_length":raw.len(),"content":String::from_utf8_lossy(&raw[..MAX_OUTPUT/2])}));
            } else {
                let event: Value = serde_json::from_slice(&raw)
                    .map_err(|e| format!("invalid JSONL line {line}: {e}"))?;
                rows.push(json!({"line":line,"event":event}));
            }
            next = line + 1;
            self.cache.insert(line, rows.last().unwrap().clone());
            bytes += raw.len();
            if bytes >= MAX_OUTPUT {
                break;
            }
        }
        Ok(
            json!({"file":name,"from_line":from,"to_line":next-1,"next_line":next,"total_lines":self.lines,"has_more":next<=self.lines,"lines":rows}),
        )
    }
}
