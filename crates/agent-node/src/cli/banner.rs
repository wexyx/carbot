use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

pub(super) fn text(server: Option<&str>, columns: usize) -> String {
    let width = columns.saturating_sub(5).clamp(12, 76);
    let data = agent_runtime::paths::data_dir();
    let home = agent_runtime::paths::user_home();
    let data = data
        .strip_prefix(&home)
        .map(|p| {
            if p.as_os_str().is_empty() {
                "~".into()
            } else {
                format!("~/{}", p.display())
            }
        })
        .unwrap_or_else(|_| data.display().to_string());
    let mut lines = vec![
        "                    __          __".to_owned(),
        "   _________ ______/ /_  ____  / /_".to_owned(),
        "  / ___/ __ `/ ___/ __ \\/ __ \\/ __/".to_owned(),
        " / /__/ /_/ / /  / /_/ / /_/ / /_".to_owned(),
        " \\___/\\__,_/_/  /_.___/\\____/\\__/".to_owned(),
        String::new(),
        format!(
            "{} · {} · {}",
            crate::app::version::DISPLAY,
            data,
            server.unwrap_or("Server 未启动")
        ),
    ];
    let mut output = format!("┌{}┐\n", "─".repeat(width + 2));
    for line in lines.drain(..) {
        let mut part = String::new();
        let mut used = 0;
        for c in line.chars() {
            let size = c.width().unwrap_or(0);
            if used + size > width {
                output.push_str(&format!("│ {}{} │\n", part, " ".repeat(width - used)));
                part.clear();
                used = 0;
            }
            if !c.is_control() {
                part.push(c);
                used += size;
            }
        }
        output.push_str(&format!(
            "│ {}{} │\n",
            part,
            " ".repeat(width - part.width())
        ));
    }
    output.push_str(&format!("└{}┘\n", "─".repeat(width + 2)));
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bordered_banner_has_aligned_metadata_without_markdown() {
        let banner = text(Some("http://127.0.0.1:8787"), 80);
        assert!(!banner.contains("```") && !banner.contains("┌ text"));
        assert!(banner.contains("http://127.0.0.1:8787"));
        assert!(banner.contains(crate::app::version::DISPLAY));
        for columns in [40, 80, 100] {
            assert!(
                text(None, columns)
                    .lines()
                    .all(|line| line.width() < columns)
            );
        }
        assert!(!banner.contains("Local data:"));
        let widths = banner
            .lines()
            .map(UnicodeWidthStr::width)
            .collect::<Vec<_>>();
        assert!(widths.iter().all(|width| *width == widths[0]));
    }
}
