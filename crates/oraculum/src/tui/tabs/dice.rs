//! Dice tab (T7_T9_PLAN §3.1): an expression input and the last ten results.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent};
use oraculum_core::dice::DiceExpr;
use oraculum_core::engine::RollResult;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::action::{Action, RollRequest, Tab, TabEvent, TabId};
use crate::tui::input::LineInput;
use crate::tui::theme;
use crate::tui::widgets::clip;

const HISTORY: usize = 10;
const HINT: &str = "e.g. 2d6+3, d66, 4d6kh3";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Edit,
    Browse,
}

pub struct DiceTab {
    input: LineInput,
    mode: Mode,
    error: Option<String>,
    /// Newest first; each is a `DiceRoll` display line.
    history: VecDeque<String>,
}

impl DiceTab {
    /// Starts in Edit so the input is ready (D-42).
    pub fn new() -> DiceTab {
        DiceTab {
            input: LineInput::new(),
            mode: Mode::Edit,
            error: None,
            history: VecDeque::new(),
        }
    }

    fn edit_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Esc => self.mode = Mode::Browse,
            KeyCode::Enter => {
                let text = self.input.text().trim().to_string();
                return match DiceExpr::parse(&text) {
                    Ok(_) => {
                        self.error = None;
                        Action::Roll(RollRequest::Dice { expr: text })
                    }
                    Err(e) => {
                        self.error = Some(e.to_string());
                        Action::None
                    }
                };
            }
            _ => {
                self.input.handle_key(key);
            }
        }
        Action::None
    }
}

impl Default for DiceTab {
    fn default() -> Self {
        DiceTab::new()
    }
}

impl Tab for DiceTab {
    fn id(&self) -> TabId {
        TabId::Dice
    }

    fn title(&self) -> &'static str {
        "Dice"
    }

    fn update(&mut self, key: KeyEvent) -> Action {
        match self.mode {
            Mode::Edit => self.edit_key(key),
            Mode::Browse => {
                if matches!(
                    key.code,
                    KeyCode::Char('/') | KeyCode::Char('i') | KeyCode::Enter
                ) {
                    self.mode = Mode::Edit;
                }
                Action::None
            }
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let width = usize::from(area.width.saturating_sub(2));
        let (before, after) = self.input.split_at_cursor();
        let mut first = vec![Span::raw("Dice: "), Span::raw(before.to_string())];
        if self.mode == Mode::Edit {
            let mut rest = after.chars();
            let at = rest.next().map_or(" ".to_string(), String::from);
            first.push(Span::styled(at, theme::active_tab()));
            first.push(Span::raw(rest.collect::<String>()));
        } else {
            first.push(Span::raw(after.to_string()));
        }
        let second = match &self.error {
            Some(e) => Line::styled(clip(&format!("! {e}"), width), theme::error()),
            None => Line::styled(HINT, theme::dim()),
        };
        let mut lines = vec![
            Line::from(first),
            second,
            Line::from(""),
            Line::styled("Last results", theme::bold()),
        ];
        lines.extend(self.history.iter().map(|h| Line::from(clip(h, width))));
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Dice")),
            area,
        );
    }

    fn captures_input(&self) -> bool {
        self.mode == Mode::Edit
    }

    fn on_event(&mut self, ev: &TabEvent) {
        match ev {
            TabEvent::Rolled(rolled) => {
                if let RollResult::Dice(roll) = &rolled.result {
                    self.history.push_front(roll.to_string());
                    self.history.truncate(HISTORY);
                }
            }
            TabEvent::RollFailed(msg) => self.error = Some(msg.clone()),
            _ => {}
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[("Enter", "roll"), ("Esc", "leave input"), ("/ i", "edit")]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_str(tab: &mut DiceTab, s: &str) {
        for c in s.chars() {
            tab.update(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn dice_invalid_expr_sets_error_and_emits_no_action() {
        for bad in ["2d", "101d6", ""] {
            let mut tab = DiceTab::new();
            type_str(&mut tab, bad);
            assert_eq!(tab.update(key(KeyCode::Enter)), Action::None, "{bad:?}");
            assert!(tab.error.is_some(), "{bad:?}");
        }
    }

    #[test]
    fn valid_expression_requests_a_roll_and_clears_the_error() {
        let mut tab = DiceTab::new();
        type_str(&mut tab, "2d");
        tab.update(key(KeyCode::Enter));
        assert!(tab.error.is_some());
        tab.update(key(KeyCode::Char('6')));
        assert_eq!(
            tab.update(key(KeyCode::Enter)),
            Action::Roll(RollRequest::Dice { expr: "2d6".into() })
        );
        assert!(tab.error.is_none());
        assert_eq!(tab.input.text(), "2d6", "text stays so Enter can repeat");
    }

    #[test]
    fn esc_leaves_the_input_and_slash_i_or_enter_return() {
        let mut tab = DiceTab::new();
        assert!(tab.captures_input());
        tab.update(key(KeyCode::Esc));
        assert!(!tab.captures_input());
        for back in [KeyCode::Char('/'), KeyCode::Char('i'), KeyCode::Enter] {
            tab.update(key(back));
            assert!(tab.captures_input(), "{back:?}");
            tab.update(key(KeyCode::Esc));
        }
        assert_eq!(tab.update(key(KeyCode::Char('x'))), Action::None);
        assert!(!tab.captures_input());
    }
}
