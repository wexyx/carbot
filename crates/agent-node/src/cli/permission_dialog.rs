use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::Value;
use uuid::Uuid;
#[derive(Clone)]
pub(super) struct Permission {
    pub(super) id: Uuid,
    pub(super) project: Uuid,
    pub(super) workspace: bool,
    pub(super) conversation: bool,
    pub(super) title: String,
    pub(super) detail: String,
}
impl Permission {
    pub(super) fn management(project: Uuid, row: &Value) -> Option<Self> {
        Some(Self {
            id: row["id"].as_str()?.parse().ok()?,
            project,
            workspace: false,
            conversation: false,
            title: format!("管理操作 · {}", row["tool"].as_str().unwrap_or("操作")),
            detail: format!(
                "目标：{}\n{}\n请求 ID：{}",
                row["client_id"]
                    .as_str()
                    .or(row["input"]["url"].as_str())
                    .unwrap_or("当前项目"),
                row["warning"].as_str().unwrap_or("允许后将执行此操作。"),
                row["id"].as_str()?
            ),
        })
    }
    pub(super) fn command(&self, allow: bool) -> String {
        format!(
            "/{} {}",
            match (self.workspace, allow) {
                (true, true) => "allow-path",
                (true, false) => "deny-path",
                (false, true) => "approve",
                (false, false) => "deny",
            },
            self.id
        )
    }
}
#[derive(Default)]
pub(super) struct PermissionDialog {
    pending: Vec<Permission>,
    input: String,
    selection: usize,
    hidden: bool,
    scroll: usize,
}
impl PermissionDialog {
    pub(super) fn update(&mut self, items: Vec<Permission>) {
        if self.pending.first().map(|p| p.id) != items.first().map(|p| p.id) {
            self.input.clear();
            self.selection = 0;
            self.hidden = false;
            self.scroll = 0;
        }
        self.pending = items;
    }
    pub(super) fn visible(&self) -> bool {
        !self.hidden && !self.pending.is_empty()
    }
    pub(super) fn scroll(&mut self, up: bool) {
        self.scroll = if up {
            self.scroll.saturating_sub(3)
        } else {
            self.scroll.saturating_add(3)
        };
    }
    pub(super) fn paste(&mut self, text: &str) {
        self.input = text.trim().chars().take(32).collect();
    }
    pub(super) fn show(&mut self) {
        self.hidden = false;
    }
    pub(super) fn text(&self) -> Option<String> {
        if !self.visible() {
            return None;
        }
        let item = &self.pending[0];
        let (cols, rows) = crossterm::terminal::size().unwrap_or((100, 28));
        if rows < 15 || cols < 50 {
            return Some("需要确认：请放大终端（至少 50×15）后操作；Esc 稍后处理。".into());
        }
        let width = usize::from(cols.saturating_sub(6)).min(82);
        let lines = super::screen::wrap(&format!("{}\n{}", item.title, item.detail), width);
        let height = usize::from(rows.saturating_sub(16)).max(1);
        let start = self.scroll.min(lines.len().saturating_sub(height));
        let detail = lines
            .into_iter()
            .skip(start)
            .take(height)
            .collect::<Vec<_>>()
            .join("\n");
        Some(format!(
            "需要你的确认（{} 项待处理）\n{}\n{}拒绝\n{}允许一次{}\n↑↓ 选择 · Enter 确认 · PgUp/PgDn 查看详情\nEsc 稍后 · 输入 确认/拒绝：{}",
            self.pending.len(),
            detail,
            if self.selection == 0 { "› " } else { "  " },
            if self.selection == 1 { "› " } else { "  " },
            if item.conversation {
                format!(
                    "\n{}本对话允许（所有命令，重置后失效）",
                    if self.selection == 2 { "› " } else { "  " }
                )
            } else {
                String::new()
            },
            self.input
        ))
    }
    pub(super) fn key(&mut self, key: KeyEvent) -> Option<(Permission, bool, bool)> {
        if !self.visible() {
            return None;
        }
        match key.code {
            KeyCode::Esc => self.hidden = true,
            KeyCode::PageUp => self.scroll(true),
            KeyCode::PageDown => self.scroll(false),
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Tab
            | KeyCode::BackTab => {
                let count = if self.pending[0].conversation { 3 } else { 2 };
                self.selection =
                    if matches!(key.code, KeyCode::Left | KeyCode::Up | KeyCode::BackTab) {
                        (self.selection + count - 1) % count
                    } else {
                        (self.selection + 1) % count
                    };
                self.input.clear();
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Char(c)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if self.input.len() < 64 {
                    self.input.push(c);
                }
            }
            KeyCode::Enter => {
                if crossterm::terminal::size().is_ok_and(|(w, h)| w < 50 || h < 15) {
                    return None;
                }
                let allow = match self.input.trim().to_lowercase().as_str() {
                    "" => self.selection != 0,
                    "本对话允许" if self.pending[0].conversation => true,
                    "确认" | "允许" | "同意" | "y" | "yes" => true,
                    "拒绝" | "取消" | "n" | "no" => false,
                    _ => return None,
                };
                self.hidden = true;
                let conversation = allow
                    && self.pending[0].conversation
                    && (self.input.trim() == "本对话允许"
                        || (self.input.trim().is_empty() && self.selection == 2));
                return Some((self.pending[0].clone(), allow, conversation));
            }
            _ => {}
        }
        None
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item() -> Permission {
        Permission {
            id: Uuid::new_v4(),
            project: Uuid::new_v4(),
            workspace: true,
            conversation: true,
            title: "outside".into(),
            detail: "/private/test".into(),
        }
    }
    #[test]
    fn vertical_navigation_selects_without_approving_and_wraps() {
        let mut dialog = PermissionDialog::default();
        dialog.update(vec![item()]);
        assert!(
            dialog
                .key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))
                .is_none()
        );
        assert_eq!(dialog.selection, 1);
        dialog.key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
        assert_eq!(dialog.selection, 1);
        dialog.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        let (_, allow, conversation) = dialog
            .key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .unwrap();
        assert!(allow && conversation);
        dialog.update(vec![item()]);
        dialog.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(dialog.selection, 2);
    }
    #[test]
    fn default_deny_and_typed_confirmation_are_explicit() {
        let mut dialog = PermissionDialog::default();
        dialog.update(vec![item()]);
        assert!(
            !dialog
                .key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .unwrap()
                .1
        );
        dialog.update(vec![item()]);
        for c in "确认".chars() {
            dialog.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert!(
            dialog
                .key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
                .unwrap()
                .1
        );
        dialog.update(vec![]);
        assert!(!dialog.visible());
    }
}
