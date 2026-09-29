use serde_json::Value;
pub(super) fn required<'a>(input: &'a Value, key: &str) -> Result<&'a str, String> {
    input[key].as_str().ok_or_else(|| format!("{key} required"))
}
