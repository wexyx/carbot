use super::CompressionStrategy;
pub(super) struct Disabled;
impl CompressionStrategy for Disabled {
    fn compress(&self, text: &str, max_bytes: usize) -> Result<String, String> {
        if text.len() > max_bytes {
            Err("CONTEXT_LIMIT: compression disabled; use /new or increase context budget".into())
        } else {
            Ok(text.into())
        }
    }
}
