//! CommonMark to terminal-friendly text. Raw ANSI/HTML never becomes a command.
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
pub(super) fn render(source: &str) -> String {
    let mut out = String::new();
    let mut list = Vec::<Option<u64>>::new();
    let mut code = false;
    let mut quote = 0usize;
    for event in Parser::new_ext(
        source,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    ) {
        match event {
            Event::Start(tag) => match tag {
                Tag::Heading { .. } => {
                    newline(&mut out);
                    out.push_str("▌ ");
                }
                Tag::Paragraph => {
                    newline(&mut out);
                    if quote > 0 {
                        out.push_str("│ ");
                    }
                }
                Tag::List(start) => {
                    newline(&mut out);
                    list.push(start);
                }
                Tag::Item => {
                    newline(&mut out);
                    out.push_str(&"  ".repeat(list.len().saturating_sub(1)));
                    match list.last_mut() {
                        Some(Some(n)) => {
                            out.push_str(&format!("{n}. "));
                            *n += 1;
                        }
                        _ => out.push_str("• "),
                    }
                }
                Tag::CodeBlock(kind) => {
                    newline(&mut out);
                    out.push_str("┌ ");
                    if let CodeBlockKind::Fenced(language) = kind {
                        out.push_str(&language);
                    }
                    out.push('\n');
                    code = true;
                }
                Tag::BlockQuote(_) => {
                    newline(&mut out);
                    quote += 1;
                }
                Tag::Table(_) | Tag::TableHead | Tag::TableRow => newline(&mut out),
                Tag::TableCell => out.push_str("│ "),
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_)
                | TagEnd::Paragraph
                | TagEnd::Item
                | TagEnd::TableHead
                | TagEnd::TableRow => newline(&mut out),
                TagEnd::CodeBlock => {
                    newline(&mut out);
                    out.push_str("└\n");
                    code = false;
                }
                TagEnd::List(_) => {
                    list.pop();
                    newline(&mut out);
                }
                TagEnd::BlockQuote(_) => {
                    quote = quote.saturating_sub(1);
                    newline(&mut out);
                }
                TagEnd::TableCell => out.push_str("  "),
                _ => {}
            },
            Event::Text(text) | Event::Code(text) => {
                if code {
                    for line in text.lines() {
                        out.push_str("│ ");
                        out.push_str(line);
                        out.push('\n');
                    }
                } else {
                    out.push_str(&text);
                }
            }
            Event::Html(text) | Event::InlineHtml(text) => out.push_str(&text),
            Event::SoftBreak | Event::HardBreak => newline(&mut out),
            Event::Rule => {
                newline(&mut out);
                out.push_str("────────────────────\n");
            }
            Event::TaskListMarker(done) => out.push_str(if done { "☑ " } else { "☐ " }),
            _ => {}
        }
    }
    out
}
fn newline(text: &mut String) {
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headings_lists_and_code_are_readable() {
        let text = render("# 标题\n\n**重点**\n\n- 条目\n\n~~~rust\nlet n=1;\n~~~");
        assert!(text.contains("▌ 标题"));
        assert!(text.contains("重点"));
        assert!(!text.contains("**"));
        assert!(text.contains("• 条目"));
        assert!(text.contains("│ let n=1;"));
    }
}
