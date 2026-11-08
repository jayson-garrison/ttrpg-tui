//! Oracle tab (T7_T9_PLAN §3.2): likelihood, an optional question, the last
//! answer with its twist, and the last ten answers.

use std::collections::VecDeque;

use crossterm::event::{KeyCode, KeyEvent};
use oraculum_core::engine::RollResult;
use oraculum_core::generator::GeneratorResult;
use oraculum_core::oracle::{Likelihood, OracleRoll, Outcome, Twist};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::action::{Action, LibraryTarget, RollRequest, Tab, TabEvent, TabId};
use crate::tui::input::LineInput;
use crate::tui::theme;
use crate::tui::widgets::{clip, result_block};

const HISTORY: usize = 10;
const LIBRARY_CATEGORY: &str = "Oracle";

struct Answer {
    roll: OracleRoll,
    twist: Twist<GeneratorResult>,
}

pub struct OracleTab {
    likelihood: Likelihood,
    question: LineInput,
    editing: bool,
    last: Option<Answer>,
    /// Newest first.
    history: VecDeque<String>,
}

impl OracleTab {
    pub fn new() -> OracleTab {
        OracleTab {
            likelihood: Likelihood::Even,
            question: LineInput::new(),
            editing: false,
            last: None,
            history: VecDeque::new(),
        }
    }

    fn step(&mut self, delta: isize) {
        let all = Likelihood::ALL;
        let at = all.iter().position(|l| *l == self.likelihood).unwrap_or(2);
        let next = at.saturating_add_signed(delta).min(all.len() - 1);
        self.likelihood = all[next];
    }

    fn ask(&mut self) -> Action {
        self.editing = false;
        let text = self.question.text().trim();
        Action::Roll(RollRequest::Oracle {
            likelihood: self.likelihood,
            question: (!text.is_empty()).then(|| text.to_string()),
        })
    }

    fn selector(&self) -> Line<'static> {
        let mut spans = Vec::new();
        for l in Likelihood::ALL {
            if !spans.is_empty() {
                spans.push(Span::raw("  "));
            }
            if l == self.likelihood {
                spans.push(Span::styled(
                    format!("[{}]", l.label()),
                    theme::active_tab(),
                ));
            } else {
                spans.push(Span::raw(l.label()));
            }
        }
        Line::from(spans)
    }

    fn question_line(&self) -> Line<'static> {
        let (before, after) = self.question.split_at_cursor();
        let mut spans = vec![Span::raw("Question: "), Span::raw(before.to_string())];
        if self.editing {
            let mut rest = after.chars();
            let at = rest.next().map_or(" ".to_string(), String::from);
            spans.push(Span::styled(at, theme::active_tab()));
            spans.push(Span::raw(rest.collect::<String>()));
        } else {
            spans.push(Span::raw(after.to_string()));
        }
        Line::from(spans)
    }

    fn answer_lines(&self) -> Vec<Line<'static>> {
        let Some(a) = &self.last else {
            return vec![Line::styled("Press Enter to ask.", theme::dim())];
        };
        let style = match a.roll.outcome {
            Outcome::ExceptionalYes => theme::ok().add_modifier(Modifier::BOLD),
            Outcome::Yes => theme::ok(),
            Outcome::ExceptionalNo => theme::error(),
            Outcome::No => Style::new().fg(Color::Red),
        };
        let mut lines = vec![Line::from(vec![
            Span::styled(a.roll.outcome.label(), style),
            Span::raw(format!("   d100: {}", a.roll.roll)),
        ])];
        match &a.twist {
            Twist::None => {}
            Twist::Event(g) => {
                lines.push(Line::styled("Twist:", theme::bold()));
                lines.extend(result_block::lines(g));
            }
            Twist::Unavailable { generator, missing } => lines.push(Line::styled(
                format!("Twist unavailable: {generator} is missing {missing}"),
                theme::warning(),
            )),
        }
        lines
    }
}

impl Default for OracleTab {
    fn default() -> Self {
        OracleTab::new()
    }
}

impl Tab for OracleTab {
    fn id(&self) -> TabId {
        TabId::Oracle
    }

    fn title(&self) -> &'static str {
        "Oracle"
    }

    fn update(&mut self, key: KeyEvent) -> Action {
        if self.editing {
            match key.code {
                KeyCode::Esc => self.editing = false,
                KeyCode::Enter => return self.ask(),
                _ => {
                    self.question.handle_key(key);
                }
            }
            return Action::None;
        }
        match key.code {
            KeyCode::Left | KeyCode::Char('h') => self.step(-1),
            KeyCode::Right | KeyCode::Char('l') => self.step(1),
            KeyCode::Char('/') => self.editing = true,
            KeyCode::Enter => return self.ask(),
            KeyCode::Char('t') => {
                return Action::OpenInLibrary(LibraryTarget::Category(LIBRARY_CATEGORY.into()))
            }
            _ => {}
        }
        Action::None
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let width = usize::from(area.width.saturating_sub(2));
        let mut lines = vec![self.selector(), self.question_line(), Line::from("")];
        lines.extend(self.answer_lines());
        lines.push(Line::from(""));
        lines.push(Line::styled("Last answers", theme::bold()));
        lines.extend(self.history.iter().map(|h| Line::from(clip(h, width))));
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("Oracle")),
            area,
        );
    }

    fn captures_input(&self) -> bool {
        self.editing
    }

    fn on_event(&mut self, ev: &TabEvent) {
        if let TabEvent::Rolled(rolled) = ev {
            if let RollResult::Oracle(res) = &rolled.result {
                self.history.push_front(format!(
                    "{} {} d100:{}",
                    res.roll.likelihood.label(),
                    res.roll.outcome.label(),
                    res.roll.roll
                ));
                self.history.truncate(HISTORY);
                self.last = Some(Answer {
                    roll: res.roll,
                    twist: res.twist.clone(),
                });
            }
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("\u{2190}/\u{2192}", "likelihood"),
            ("/", "question"),
            ("Enter", "ask"),
            ("t", "library"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testkit::{empty_engine, engine};
    use crossterm::event::KeyModifiers;
    use oraculum_core::engine::Engine;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn screen(tab: &OracleTab) -> String {
        let mut term = Terminal::new(TestBackend::new(60, 18)).unwrap();
        term.draw(|f| tab.render(f, f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..18)
            .map(|y| (0..60).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Press Enter, run the request on `engine`, and deliver the result.
    fn ask(tab: &mut OracleTab, engine: &mut Engine) {
        let Action::Roll(RollRequest::Oracle {
            likelihood,
            question,
        }) = tab.update(key(KeyCode::Enter))
        else {
            unreachable!("Enter asks")
        };
        let rolled = engine.ask(likelihood, question.as_deref());
        tab.on_event(&TabEvent::Rolled(Box::new(rolled)));
    }

    #[test]
    fn arrows_change_likelihood_and_clamp_at_both_ends() {
        let mut tab = OracleTab::new();
        for _ in 0..9 {
            tab.update(key(KeyCode::Right));
        }
        assert_eq!(tab.likelihood, Likelihood::VeryLikely);
        for _ in 0..9 {
            tab.update(key(KeyCode::Char('h')));
        }
        assert_eq!(tab.likelihood, Likelihood::VeryUnlikely);
        tab.update(key(KeyCode::Char('l')));
        assert_eq!(tab.likelihood, Likelihood::Unlikely);
        assert!(screen(&tab).contains("[Unlikely]"));
    }

    #[test]
    fn enter_asks_with_the_trimmed_question_or_none() {
        let mut tab = OracleTab::new();
        assert_eq!(
            tab.update(key(KeyCode::Enter)),
            Action::Roll(RollRequest::Oracle {
                likelihood: Likelihood::Even,
                question: None
            })
        );
        tab.update(key(KeyCode::Char('/')));
        assert!(tab.captures_input());
        for c in " Is it open? ".chars() {
            tab.update(key(KeyCode::Char(c)));
        }
        assert_eq!(
            tab.update(key(KeyCode::Enter)),
            Action::Roll(RollRequest::Oracle {
                likelihood: Likelihood::Even,
                question: Some("Is it open?".into())
            })
        );
        assert!(!tab.captures_input(), "Enter ends editing");
    }

    #[test]
    fn esc_ends_editing_and_keys_are_text_while_editing() {
        let mut tab = OracleTab::new();
        tab.update(key(KeyCode::Char('/')));
        tab.update(key(KeyCode::Char('t')));
        tab.update(key(KeyCode::Char('h')));
        assert_eq!(tab.question.text(), "th");
        assert_eq!(tab.likelihood, Likelihood::Even);
        tab.update(key(KeyCode::Esc));
        assert!(!tab.captures_input());
    }

    #[test]
    fn t_opens_the_oracle_category_in_the_library() {
        let mut tab = OracleTab::new();
        assert_eq!(
            tab.update(key(KeyCode::Char('t'))),
            Action::OpenInLibrary(LibraryTarget::Category("Oracle".into()))
        );
    }

    #[test]
    fn answer_shows_label_and_d100_in_plain_text() {
        let mut tab = OracleTab::new();
        let mut e = engine(&[30]);
        ask(&mut tab, &mut e);
        let s = screen(&tab);
        assert!(s.contains("Yes"), "{s}");
        assert!(s.contains("d100: 30"), "{s}");
        assert!(s.contains("Even Yes d100:30"), "history line\n{s}");
        assert!(!s.contains("Twist"), "{s}");
    }

    #[test]
    fn a_double_shows_the_twist_block() {
        let mut tab = OracleTab::new();
        // 22 is a double; the event generator rolls words -> 2 (beta).
        let mut e = engine(&[22, 2]);
        ask(&mut tab, &mut e);
        let s = screen(&tab);
        assert!(s.contains("d100: 22"), "{s}");
        assert!(s.contains("Twist:"), "{s}");
        assert!(s.contains("beta"), "{s}");
    }

    #[test]
    fn a_missing_event_generator_says_so_instead_of_failing() {
        let mut tab = OracleTab::new();
        let mut e = empty_engine(&[22]);
        ask(&mut tab, &mut e);
        let s = screen(&tab);
        assert!(s.contains("Twist unavailable: event is missing"), "{s}");
        assert!(s.contains("Yes"), "{s}");
    }

    #[test]
    fn history_keeps_ten_newest_first() {
        let mut tab = OracleTab::new();
        // No doubles, so each ask consumes one roll.
        let mut e = engine(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13]);
        for _ in 0..12 {
            ask(&mut tab, &mut e);
        }
        assert_eq!(tab.history.len(), 10);
        assert!(tab.history[0].ends_with("d100:13"), "{:?}", tab.history);
        assert!(tab.history[9].ends_with("d100:3"), "{:?}", tab.history);
    }
}
