use super::{
    CompressionStrategy,
    contract::{NOTICE, prefix, suffix},
};
pub(super) struct Extractive;
impl CompressionStrategy for Extractive {
    fn compress(&self, text: &str, max_bytes: usize) -> Result<String, String> {
        if text.len() <= max_bytes {
            return Ok(text.into());
        }
        if max_bytes < NOTICE.len() {
            return Err("CONTEXT_LIMIT: budget too small".into());
        }
        let available = max_bytes - NOTICE.len();
        let head = available / 4;
        Ok(format!(
            "{}{NOTICE}{}",
            prefix(text, head),
            suffix(text, available - head)
        ))
    }
}
