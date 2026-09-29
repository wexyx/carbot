use super::{
    CompressionStrategy,
    contract::{NOTICE, suffix},
};
pub(super) struct Window;
impl CompressionStrategy for Window {
    fn compress(&self, text: &str, max_bytes: usize) -> Result<String, String> {
        if text.len() <= max_bytes {
            return Ok(text.into());
        }
        if max_bytes < NOTICE.len() {
            return Err("CONTEXT_LIMIT: budget too small".into());
        }
        Ok(format!(
            "{NOTICE}{}",
            suffix(text, max_bytes - NOTICE.len())
        ))
    }
}
