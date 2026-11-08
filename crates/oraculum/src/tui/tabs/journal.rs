//! Journal tab (T7_T9_PLAN §3.3): the session's entries, a note input, export.
//!
//! "Following" is derived: the selection sits on the last entry, and a new
//! entry then moves the selection with it.

use std::cell::Cell;

use crossterm::event::{KeyCode, KeyEvent};
use oraculum_core::journal::{format_utc, Entry, Kind};
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::tui::action::{Action, Tab, TabEvent, TabId};
use crate::tui::input::LineInput;
use crate::tui::theme;
use crate::tui::widgets::clip;

pub struct JournalTab {
    entries: Vec<Entry>,
    selected: usize,
    note: Option<LineInput>,
    /// First visible row and visible rows, kept by `render` so paging and
    /// scrolling follow what is on screen.
    top: Cell<usize>,
    rows: Cell<usize>,
}

impl JournalTab {
    pub fn new() -> JournalTab {
        JournalTab {
            entries: Vec::new(),
            selected: 0,
            note: None,
            top: Cell::new(0),
            rows: Cell::new(10),
        }
    }

    fn last(&self) -> usize {
        self.entries.len().saturating_sub(1)
    }

    fn at_end(&self) -> bool {
        self.selected >= self.last()
    }

    fn move_to(&mut self, index: usize) {
        self.selected = index.min(self.last());
    }

    fn page(&self) -> usize {
        self.rows.get().saturating_sub(1).max(1)
    }

    fn note_key(&mut self, key: KeyEvent) -> Action {
        let Some(input) = self.note.as_mut() else {
            return Action::None;
        };
        match key.code {
            KeyCode::Esc => self.note = None,
            KeyCode::Enter => {
                let text = input.text().trim().to_string();
                self.note = None;
                if !text.is_empty() {
                    return Action::AppendNote(text);
                }
            }
            _ => {
                input.handle_key(key);
            }
        }
        Action::None
    }
}

impl Default for JournalTab {
    fn default() -> Self {
        JournalTab::new()
    }
}

/// `HH:MM  kind  input -> result`, multi-line results cut to the first line.
fn row_text(e: &Entry) -> String {
    let time = format_utc(e.ts);
    let time = time.get(11..).unwrap_or(&time);
    let first = e.result.lines().next().unwrap_or("");
    let more = if e.result.trim_end().contains('\n') {
        "\u{2026}"
    } else {
        ""
    };
    match e.kind {
        Kind::Note => format!("{time}  note  {first}{more}"),
        k => format!("{time}  {}  {} -> {first}{more}", k.label(), e.input),
    }
}

impl Tab for JournalTab {
    fn id(&self) -> TabId {
        TabId::Journal
    }

    fn title(&self) -> &'static str {
        "Journal"
    }

    fn update(&mut self, key: KeyEvent) -> Action {
        if self.note.is_some() {
            return self.note_key(key);
        }
        match key.code {
            KeyCode::Up => self.move_to(self.selected.saturating_sub(1)),
            KeyCode::Down => self.move_to(self.selected + 1),
            KeyCode::PageUp => self.move_to(self.selected.saturating_sub(self.page())),
            KeyCode::PageDown => self.move_to(self.selected + self.page()),
            KeyCode::Home => self.move_to(0),
            KeyCode::End => self.move_to(self.last()),
            KeyCode::Char('n') => self.note = Some(LineInput::new()),
            KeyCode::Char('x') => return Action::ExportJournal,
            _ => {}
        }
        Action::None
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let inner_w = usize::from(area.width.saturating_sub(2));
        let inner_h = usize::from(area.height.saturating_sub(2));
        let rows = inner_h
            .saturating_sub(usize::from(self.note.is_some()))
            .max(1);
        self.rows.set(rows);
        // Keep the selection on screen; otherwise leave the window alone.
        let mut top = self.top.get().min(self.entries.len().saturating_sub(rows));
        if self.selected < top {
            top = self.selected;
        } else if self.selected >= top + rows {
            top = self.selected + 1 - rows;
        }
        self.top.set(top);

        let mut lines: Vec<Line> = self
            .entries
            .iter()
            .enumerate()
            .skip(top)
            .take(rows)
            .map(|(i, e)| {
                let text = clip(&row_text(e), inner_w);
                if i == self.selected {
                    Line::styled(text, theme::active_tab())
                } else {
                    Line::from(text)
                }
            })
            .collect();
        if self.entries.is_empty() {
            lines.push(Line::styled(
                "No entries yet. Roll something, or press n for a note.",
                theme::dim(),
            ));
        }
        if let Some(input) = &self.note {
            let (before, after) = input.split_at_cursor();
            let mut rest = after.chars();
            let at = rest.next().map_or(" ".to_string(), String::from);
            lines.truncate(rows);
            lines.push(Line::from(vec![
                Span::raw("Note: "),
                Span::raw(before.to_string()),
                Span::styled(at, theme::active_tab()),
                Span::raw(rest.collect::<String>()),
            ]));
        }
        let title = format!("Journal ({} entries)", self.entries.len());
        frame.render_widget(
            Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title)),
            area,
        );
    }

    fn captures_input(&self) -> bool {
        self.note.is_some()
    }

    fn on_event(&mut self, ev: &TabEvent) {
        match ev {
            TabEvent::JournalLoaded(entries, _) => {
                self.entries = entries.clone();
                self.selected = self.last();
            }
            TabEvent::JournalAppended(entry) => {
                let follow = self.at_end();
                self.entries.push(entry.clone());
                if follow {
                    self.selected = self.last();
                }
            }
            _ => {}
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("n", "note"),
            ("x", "export"),
            ("\u{2191}/\u{2193} PgUp/PgDn", "scroll"),
            ("Home/End", "first / last"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use serde_json::Value;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn entry(n: u64, kind: Kind, input: &str, result: &str) -> Entry {
        // 2023-11-14 22:13 UTC + n minutes.
        Entry::new(1_700_000_000 + n * 60, kind, input, result, Value::Null)
    }

    fn tab_with(n: u64) -> JournalTab {
        let mut tab = JournalTab::new();
        let entries = (0..n)
            .map(|i| entry(i, Kind::Dice, &format!("d{i}"), &format!("r{i}")))
            .collect();
        tab.on_event(&TabEvent::JournalLoaded(entries, vec![]));
        tab
    }

    fn screen(tab: &JournalTab, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(60, h)).unwrap();
        term.draw(|f| tab.render(f, f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..60).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn rows_show_time_kind_input_and_result_and_notes_show_text() {
        let mut tab = JournalTab::new();
        tab.on_event(&TabEvent::JournalLoaded(
            vec![
                entry(0, Kind::Dice, "3d6+2", "3d6+2 => [1, 2, 3] +2 = 8"),
                entry(1, Kind::Note, "", "met the baron"),
                entry(2, Kind::Oracle, "Even", "Even: Yes (d100: 22)\nTwist: beta"),
            ],
            vec![],
        ));
        let s = screen(&tab, 10);
        assert!(s.contains("Journal (3 entries)"), "{s}");
        assert!(
            s.contains("22:13  dice  3d6+2 -> 3d6+2 => [1, 2, 3] +2 = 8"),
            "{s}"
        );
        assert!(s.contains("22:14  note  met the baron"), "{s}");
        assert!(
            s.contains("22:15  oracle  Even -> Even: Yes (d100: 22)\u{2026}"),
            "{s}"
        );
    }

    #[test]
    fn n_opens_a_note_input_and_enter_emits_the_trimmed_text() {
        let mut tab = tab_with(1);
        assert!(!tab.captures_input());
        tab.update(key(KeyCode::Char('n')));
        assert!(tab.captures_input());
        for c in "  hello x ".chars() {
            tab.update(key(KeyCode::Char(c)));
        }
        assert!(screen(&tab, 10).contains("Note: "));
        assert_eq!(
            tab.update(key(KeyCode::Enter)),
            Action::AppendNote("hello x".into())
        );
        assert!(!tab.captures_input());
    }

    #[test]
    fn an_empty_note_just_closes_and_esc_cancels() {
        let mut tab = tab_with(1);
        tab.update(key(KeyCode::Char('n')));
        assert_eq!(tab.update(key(KeyCode::Enter)), Action::None);
        assert!(!tab.captures_input());
        tab.update(key(KeyCode::Char('n')));
        tab.update(key(KeyCode::Char('a')));
        assert_eq!(tab.update(key(KeyCode::Esc)), Action::None);
        assert!(!tab.captures_input());
    }

    #[test]
    fn x_requests_an_export() {
        let mut tab = tab_with(0);
        assert_eq!(tab.update(key(KeyCode::Char('x'))), Action::ExportJournal);
    }

    #[test]
    fn the_list_follows_new_entries_only_while_at_the_end() {
        let mut tab = tab_with(5);
        assert_eq!(tab.selected, 4);
        tab.on_event(&TabEvent::JournalAppended(entry(9, Kind::Note, "", "a")));
        assert_eq!(tab.selected, 5, "at the end: follows");
        tab.update(key(KeyCode::Up));
        tab.update(key(KeyCode::Up));
        assert_eq!(tab.selected, 3);
        tab.on_event(&TabEvent::JournalAppended(entry(10, Kind::Note, "", "b")));
        assert_eq!(tab.selected, 3, "scrolled up: stays");
        tab.update(key(KeyCode::End));
        tab.on_event(&TabEvent::JournalAppended(entry(11, Kind::Note, "", "c")));
        assert_eq!(tab.selected, 7, "End re-attaches");
    }

    #[test]
    fn paging_moves_by_a_page_and_clamps() {
        let mut tab = tab_with(40);
        screen(&tab, 12); // 10 body rows -> page of 9
        tab.update(key(KeyCode::PageUp));
        assert_eq!(tab.selected, 39 - 9);
        tab.update(key(KeyCode::Home));
        assert_eq!(tab.selected, 0);
        tab.update(key(KeyCode::Up));
        assert_eq!(tab.selected, 0);
        tab.update(key(KeyCode::PageDown));
        assert_eq!(tab.selected, 9);
        for _ in 0..10 {
            tab.update(key(KeyCode::PageDown));
        }
        assert_eq!(tab.selected, 39);
    }

    #[test]
    fn the_window_keeps_the_selection_visible() {
        let mut tab = tab_with(40);
        let s = screen(&tab, 12);
        assert!(s.contains("-> r39") && !s.contains("-> r0\n"), "{s}");
        tab.update(key(KeyCode::Home));
        let s = screen(&tab, 12);
        assert!(s.contains("d0 -> r0"), "{s}");
        assert!(!s.contains("r39"), "{s}");
    }

    #[test]
    fn an_empty_journal_renders_and_navigates_without_panic() {
        let mut tab = JournalTab::new();
        for code in [KeyCode::Up, KeyCode::Down, KeyCode::PageUp, KeyCode::End] {
            tab.update(key(code));
        }
        assert!(screen(&tab, 6).contains("No entries yet"));
    }
}
