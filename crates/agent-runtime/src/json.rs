//! Recovering JSON from a model answer, when the answer is not nothing but JSON.
//!
//! A prompt that says "return only JSON" is a request, not a guarantee: models wrap
//! JSON in a markdown fence, prefix it with a sentence of prose, or append a note
//! after it. Parsing the whole answer strictly rejects results that are plainly well
//! formed, so every site that reads JSON back out of a model goes through here.

/// The first balanced JSON object or array in `text`, or `None` if there is none.
///
/// Brace counting respects string literals and escapes, so a `{` inside a quoted
/// value does not end the scan early.
pub fn first_value(text: &str) -> Option<&str> {
    let bytes = text.as_bytes();
    let start = text.find(['{', '['])?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (index, byte) in bytes.iter().enumerate().skip(start) {
        if in_string {
            match byte {
                _ if escaped => escaped = false,
                b'\\' => escaped = true,
                b'"' => in_string = false,
                _ => {}
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => depth += 1,
            b'}' | b']' => {
                depth -= 1;
                if depth == 0 {
                    return text.get(start..=index);
                }
            }
            _ => {}
        }
    }
    None
}

/// Strip a markdown fence if the whole answer is one, then extract the JSON inside.
///
/// The fence is only unwrapped when it encloses the answer, so prose that merely
/// quotes a fence cannot change what is parsed.
pub fn unwrap_fence(text: &str) -> &str {
    let trimmed = text.trim();
    let Some(after_open) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    // Skip the optional language tag on the opening fence line.
    let body = match after_open.find('\n') {
        Some(newline) => &after_open[newline + 1..],
        None => return trimmed,
    };
    match body.trim_end().strip_suffix("```") {
        Some(inner) => inner.trim(),
        // An unterminated fence is a model that ran out of output, not a wrapper.
        None => trimmed,
    }
}

/// Parse the first JSON value in a model answer, ignoring anything around it.
pub fn parse(text: &str) -> Result<serde_json::Value, String> {
    let candidate = first_value(unwrap_fence(text)).ok_or("no JSON value found in the answer")?;
    serde_json::from_str(candidate).map_err(|error| error.to_string())
}

/// Parse the first JSON value, returning `None` when the answer holds none.
///
/// Use this when "no JSON" is an ordinary outcome, such as a model that was asked
/// for a tool call but answered normally instead.
pub fn parse_opt(text: &str) -> Option<serde_json::Value> {
    parse(text).ok()
}

/// A short excerpt of what the model actually returned, for error messages.
pub fn excerpt(text: &str, limit: usize) -> String {
    let text = text.trim();
    let head: String = text.chars().take(limit).collect();
    if head.len() < text.len() {
        format!("{head}…")
    } else {
        head
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn plain_json_is_returned_unchanged() {
        assert_eq!(unwrap_fence(r#"{"a":1}"#), r#"{"a":1}"#);
        assert_eq!(first_value(r#"{"a":1}"#), Some(r#"{"a":1}"#));
        assert_eq!(parse(r#" {"a":1} "#).unwrap()["a"], 1);
        assert_eq!(parse_opt(r#" {"a":1} "#).unwrap()["a"], 1);
    }
    #[test]
    fn a_markdown_fence_is_unwrapped_with_or_without_a_language() {
        assert_eq!(unwrap_fence("```json\n{\"a\":1}\n```"), r#"{"a":1}"#);
        assert_eq!(unwrap_fence("```\n[1,2]\n```"), "[1,2]");
        assert_eq!(parse("```json\n{\"a\":2}\n```").unwrap()["a"], 2);
        // Fence text with no closing fence stays as the model wrote it.
        assert_eq!(unwrap_fence("```json\n{\"a\":1}"), "```json\n{\"a\":1}");
    }
    #[test]
    fn prose_around_the_value_is_ignored() {
        let answer = "Here is the plan:\n```json\n{\"assignments\":[]}\n```\nHope that helps!";
        assert_eq!(parse(answer).unwrap()["assignments"], serde_json::json!([]));
        // Prose with no fence at all.
        let bare = r#"Sure. {"a":3} — let me know if you want changes."#;
        assert_eq!(parse(bare).unwrap()["a"], 3);
    }
    #[test]
    fn braces_inside_strings_do_not_end_the_value_early() {
        let answer = r#"{"instruction":"use {braces} and \"quotes\"","member":"bob"}"#;
        let plan = parse(answer).unwrap();
        assert_eq!(plan["instruction"], "use {braces} and \"quotes\"");
        assert_eq!(plan["member"], "bob");
        // An escaped backslash before a quote must not escape the wrong character.
        let escaped = r#"{"instruction":"path C:\\\\ then \"x\""}"#;
        assert_eq!(
            parse(escaped).unwrap()["instruction"],
            r#"path C:\\ then "x""#
        );
    }
    #[test]
    fn an_array_value_is_recognised() {
        assert_eq!(first_value("noise [1,2,3] noise"), Some("[1,2,3]"));
        assert_eq!(parse("[1,2]").unwrap(), serde_json::json!([1, 2]));
    }
    #[test]
    fn an_answer_with_no_json_is_reported_rather_than_panicking() {
        assert_eq!(first_value("I could not decide."), None);
        assert_eq!(first_value(""), None);
        assert_eq!(first_value("{}"), Some("{}"));
        // Balanced braces that never close are not a value.
        assert_eq!(first_value(r#"{"a":1"#), None);
        assert!(parse("not json at all").is_err());
        assert_eq!(parse_opt("not json at all"), None);
    }
    #[test]
    fn excerpts_are_trimmed_and_marked_when_cut() {
        assert_eq!(excerpt("  hi  ", 200), "hi");
        assert_eq!(excerpt("abcdef", 3), "abc…");
        assert_eq!(excerpt("abc", 3), "abc");
        assert_eq!(excerpt("", 3), "");
    }
}
