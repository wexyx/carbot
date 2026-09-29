use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub(super) const COMMANDS: &[&str] = &[
    "/help",
    "/update",
    "/attach",
    "/detach",
    "/manage",
    "/back",
    "/members",
    "/agent-config",
    "/connections",
    "/disconnect",
    "/reconnect",
    "/connect",
    "/server",
    "/namespace",
    "/capabilities",
    "/skills",
    "/tool-library",
    "/history",
    "/tools",
    "/new",
    "/permissions",
    "/allowlist",
    "/resume",
    "/chat",
    "/agents",
    "/add-agent",
    "/remove-agent",
    "/agent",
    "/group",
    "/admin",
    "/admin-config",
    "/project",
    "/interrupt",
    "/approve",
    "/deny",
    "/allow-path",
    "/deny-path",
    "/exit",
];
#[derive(Default)]
pub(super) struct Editor {
    buffer: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    index: Option<usize>,
    draft: String,
}
impl Editor {
    pub(super) fn text(&self) -> String {
        self.buffer.iter().collect()
    }
    pub(super) fn before_cursor(&self) -> String {
        self.buffer[..self.cursor].iter().collect()
    }
    pub(super) fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
        self.index = None;
    }
    pub(super) fn insert(&mut self, text: &str) {
        for c in text.chars().filter(|c| !c.is_control() || *c == '\n') {
            if self.buffer.len() >= 16384 {
                break;
            }
            self.buffer.insert(self.cursor, c);
            self.cursor += 1;
        }
    }
    pub(super) fn submit(&mut self, remember: bool) -> String {
        let value = self.text();
        if remember && !value.trim().is_empty() && self.history.last() != Some(&value) {
            self.history.push(value.clone());
            if self.history.len() > 200 {
                self.history.remove(0);
            }
        }
        self.clear();
        value
    }
    pub(super) fn suggestions(&self) -> Vec<&'static str> {
        let value = self.text();
        if !value.starts_with('/') || value.contains(' ') {
            return vec![];
        }
        COMMANDS
            .iter()
            .copied()
            .filter(|s| s.starts_with(&value))
            .collect()
    }
    pub(super) fn key(&mut self, key: KeyEvent, secret: bool) {
        match key.code {
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => self.cursor = 0,
            KeyCode::Char('e') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cursor = self.buffer.len()
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.buffer.drain(..self.cursor);
                self.cursor = 0;
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.insert(&c.to_string())
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.buffer.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.buffer.len(),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                self.buffer.remove(self.cursor);
            }
            KeyCode::Delete if self.cursor < self.buffer.len() => {
                self.buffer.remove(self.cursor);
            }
            KeyCode::Enter
                if key
                    .modifiers
                    .intersects(KeyModifiers::ALT | KeyModifiers::SHIFT) =>
            {
                self.insert("\n")
            }
            KeyCode::Tab if !secret => {
                if let Some(candidate) = self.suggestions().first().copied() {
                    self.clear();
                    self.insert(candidate);
                    self.insert(" ");
                }
            }
            KeyCode::Up | KeyCode::PageUp if !secret && !self.history.is_empty() => {
                if self.index.is_none() {
                    self.draft = self.text();
                }
                let i = self.index.unwrap_or(self.history.len()).saturating_sub(1);
                self.index = Some(i);
                self.buffer = self.history[i].chars().collect();
                self.cursor = self.buffer.len();
            }
            KeyCode::Down | KeyCode::PageDown if !secret => {
                if let Some(i) = self.index {
                    let next = i + 1;
                    if next < self.history.len() {
                        self.index = Some(next);
                        self.buffer = self.history[next].chars().collect();
                    } else {
                        self.index = None;
                        self.buffer = self.draft.chars().collect();
                    }
                    self.cursor = self.buffer.len();
                }
            }
            _ => {}
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_editing_history_and_completion() {
        let mut e = Editor::default();
        e.insert("你好");
        e.key(KeyCode::Left.into(), false);
        e.insert("，");
        assert_eq!(e.text(), "你，好");
        e.submit(true);
        e.key(KeyCode::Up.into(), false);
        assert_eq!(e.text(), "你，好");
        e.clear();
        e.insert("/admin-c");
        e.key(KeyCode::Tab.into(), false);
        assert_eq!(e.text(), "/admin-config ");
        e.clear();
        e.insert("secret");
        e.submit(false);
        e.key(KeyCode::Up.into(), true);
        assert_eq!(e.text(), "");
    }
}
