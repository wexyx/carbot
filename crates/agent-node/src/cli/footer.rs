use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

fn clip(text: &str, width: usize) -> String {
    let mut used = 0;
    text.chars()
        .filter(|c| !c.is_control())
        .take_while(|c| {
            used += c.width().unwrap_or(0);
            used <= width
        })
        .collect()
}

/// Left-hand session metadata and a right-aligned update notice on one terminal row.
pub(super) fn layout(session: &str, update: &str, width: usize) -> (String, String) {
    let right = clip(update, width);
    if right.is_empty() {
        return (clip(&format!("│ {session}"), width), right);
    }
    let available = width.saturating_sub(right.width());
    let mut left = clip(&format!("│ {session}"), available.saturating_sub(2));
    left.push_str(&" ".repeat(available.saturating_sub(left.width())));
    (left, right)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_is_flush_right_and_never_adds_a_line() {
        let (left, right) = layout("项目 · 请求批准", "有更新 v1.2.3 · /update", 80);
        assert!(left.starts_with("│ 项目"));
        assert_eq!(left.width() + right.width(), 80);
        assert_eq!(right, "有更新 v1.2.3 · /update");
        for width in [12, 24, 40] {
            let (left, right) = layout(
                "很长的项目名称".repeat(20).as_str(),
                "有更新 v1.2.3 · /update",
                width,
            );
            assert!(left.width() + right.width() <= width);
        }
        assert_eq!(layout("项目 · 请求批准", "", 80).0, "│ 项目 · 请求批准");
    }
}
