use serde_json::Value;
pub(super) const DEFAULT: &str = "结论先行，只回答最重要的信息；默认简洁，不重复背景或长篇解释。除非用户要求展开，否则只给必要的结论、风险和下一步。群聊中不要复述其他成员已说过的内容。";
pub(super) fn resolve(input: &Value, previous: Option<&Value>) -> Result<String, String> {
    match input.get("response_instructions") {
        Some(value) => {
            let text = value.as_str().ok_or("response_instructions must be text")?;
            if text.len() > 8192 {
                return Err("回复要求不能超过 8192 字节".into());
            }
            Ok(text.to_owned())
        }
        None => Ok(previous
            .and_then(|v| v["response_instructions"].as_str())
            .unwrap_or(DEFAULT)
            .into()),
    }
}
