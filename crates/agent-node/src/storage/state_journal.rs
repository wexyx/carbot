//! One committed transaction per JSONL line; only changed records are serialized.
use super::Data;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs::OpenOptions,
    io::{BufRead, BufReader, Seek, SeekFrom, Write},
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub(super) struct Change {
    pub collection: String,
    pub key: String,
    pub value: Value,
    pub deleted: bool,
}
#[derive(Serialize, Deserialize)]
struct Transaction {
    version: u32,
    sequence: u64,
    changes: Vec<Change>,
}

fn open(path: &Path) -> std::io::Result<std::fs::File> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

pub(super) fn replay(path: &Path, data: &mut Data) -> Result<(), String> {
    let file = open(path).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(file);
    let mut offset = 0;
    let mut line = Vec::new();
    loop {
        line.clear();
        let count = reader
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if line.last() != Some(&b'\n') {
            // A crash before the commit delimiter leaves an uncommitted tail.
            reader
                .get_ref()
                .set_len(offset)
                .and_then(|_| reader.get_ref().sync_data())
                .map_err(|e| e.to_string())?;
            eprintln!("Recovered incomplete state.jsonl transaction at byte {offset}");
            break;
        }
        let row: Transaction = serde_json::from_slice(&line)
            .map_err(|e| format!("invalid state.jsonl at byte {offset}: {e}"))?;
        if row.version != 1 || row.sequence != data.sequence + 1 {
            return Err(format!(
                "invalid state.jsonl sequence/version at byte {offset}"
            ));
        }
        for change in row.changes {
            if change.deleted {
                data.remove(&change.collection, &change.key);
            } else {
                data.set(&change.collection, &change.key, change.value);
            }
        }
        data.sequence = row.sequence;
        offset += count as u64;
    }
    reader.get_ref().sync_all().map_err(|e| e.to_string())?;
    #[cfg(unix)]
    std::fs::File::open(path.parent().ok_or("missing journal parent")?)
        .and_then(|dir| dir.sync_all())
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn append(path: &Path, sequence: u64, changes: Vec<Change>) -> Result<(), String> {
    let mut bytes = serde_json::to_vec(&Transaction {
        version: 1,
        sequence,
        changes,
    })
    .map_err(|e| e.to_string())?;
    bytes.push(b'\n');
    let mut file = open(path).map_err(|e| e.to_string())?;
    let length = file.seek(SeekFrom::End(0)).map_err(|e| e.to_string())?;
    if let Err(error) = file.write_all(&bytes).and_then(|_| file.sync_data()) {
        // Never append after an uncertain write: poison the Store if rollback fails.
        if file.set_len(length).and_then(|_| file.sync_data()).is_err() {
            panic!("state journal rollback failed; restart required: {error}");
        }
        return Err(format!("append state.jsonl: {error}"));
    }
    Ok(())
}
