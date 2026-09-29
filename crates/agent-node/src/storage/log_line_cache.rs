use serde_json::Value;
use std::collections::{HashMap, VecDeque};
/// A bounded per-file line cache; full raw journals never live here.
#[derive(Default)]
pub(super) struct LineCache {
    rows: HashMap<u64, (Value, usize)>,
    order: VecDeque<u64>,
    bytes: usize,
}
impl LineCache {
    pub(super) fn get(&mut self, line: u64) -> Option<Value> {
        let value = self.rows.get(&line)?.0.clone();
        self.order.retain(|n| *n != line);
        self.order.push_back(line);
        Some(value)
    }
    pub(super) fn insert(&mut self, line: u64, value: Value) {
        let size = value.to_string().len() + 128;
        if size > 512 * 1024 {
            return;
        }
        if let Some((_, size)) = self.rows.remove(&line) {
            self.bytes -= size;
        }
        self.order.retain(|n| *n != line);
        while self.bytes + size > 512 * 1024 || self.rows.len() >= 512 {
            if let Some(old) = self.order.pop_front() {
                if let Some((_, size)) = self.rows.remove(&old) {
                    self.bytes -= size;
                }
            } else {
                break;
            }
        }
        self.bytes += size;
        self.order.push_back(line);
        self.rows.insert(line, (value, size));
    }
}
