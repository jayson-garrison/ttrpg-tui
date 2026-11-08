//! Library tab (T7_T9_PLAN §3.4, T9.1): browse, filter, categories, detail and
//! entries views, roll and repeat. Slots and warnings arrive with T9.2.

use std::cell::Cell;
use std::rc::Rc;

use crossterm::event::{KeyCode, KeyEvent};
use oraculum_core::catalog::{Catalog, Filter, ItemKind, ItemSummary};
use oraculum_core::engine::RollResult;
use oraculum_core::generator::SlotSource;
use oraculum_core::table::TableForm;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::tui::action::{Action, LibraryTarget, RollRequest, Tab, TabEvent, TabId, Toast};
use crate::tui::input::LineInput;
use crate::tui::theme;
use crate::tui::widgets::{clip, result_block};

const MIN_LIST_WIDTH: u16 = 22;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Row {
    Header(String),
    Item(ItemSummary),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum View {
    Detail,
    Entries,
}

/// The latest table or generator roll, and the item it came from.
struct Last {
    kind: ItemKind,
    id: String,
    result: RollResult,
}

pub struct LibraryTab {
    catalog: Rc<Catalog>,
    filtering: bool,
    filter: LineInput,
    category: Option<String>,
    rows: Vec<Row>,
    /// Index into `rows`; always an `Item` when there is one.
    selected: usize,
    view: View,
    entries_scroll: usize,
    last: Option<Last>,
    /// First visible row and visible rows, kept by `render`.
    top: Cell<usize>,
    visible: Cell<usize>,
}

impl LibraryTab {
    pub fn new(catalog: Rc<Catalog>) -> LibraryTab {
        let mut tab = LibraryTab {
            catalog,
            filtering: false,
            filter: LineInput::new(),
            category: None,
            rows: Vec::new(),
            selected: 0,
            view: View::Detail,
            entries_scroll: 0,
            last: None,
            top: Cell::new(0),
            visible: Cell::new(10),
        };
        tab.rebuild();
        tab
    }

    fn current(&self) -> Option<&ItemSummary> {
        match self.rows.get(self.selected) {
            Some(Row::Item(item)) => Some(item),
            _ => None,
        }
    }

    /// Rebuild the rows from the filter and keep the same item selected when
    /// it is still listed (never cached across a filter change).
    fn rebuild(&mut self) {
        let keep = self.current().map(|i| (i.kind, i.id.clone()));
        let text = self.filter.text().trim();
        let found = self.catalog.search(&Filter {
            text: (!text.is_empty()).then(|| text.to_string()),
            category: self.category.clone(),
            kind: None,
        });
        let mut groups: std::collections::BTreeMap<&str, Vec<&ItemSummary>> = Default::default();
        for item in found {
            groups.entry(item.category.as_str()).or_default().push(item);
        }
        self.rows = groups
            .into_iter()
            .flat_map(|(category, items)| {
                std::iter::once(Row::Header(category.to_string()))
                    .chain(items.into_iter().cloned().map(Row::Item))
            })
            .collect();
        self.selected = keep
            .and_then(|(kind, id)| self.position(kind, &id))
            .or_else(|| self.first_item())
            .unwrap_or(0);
        self.reset_view();
    }

    fn reset_view(&mut self) {
        self.view = View::Detail;
        self.entries_scroll = 0;
    }

    fn position(&self, kind: ItemKind, id: &str) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| matches!(r, Row::Item(i) if i.kind == kind && i.id == id))
    }

    fn first_item(&self) -> Option<usize> {
        self.rows.iter().position(|r| matches!(r, Row::Item(_)))
    }

    /// Move by `delta` items, skipping headers, clamped at both ends.
    fn move_by(&mut self, delta: isize) {
        let items: Vec<usize> = self
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| matches!(r, Row::Item(_)).then_some(i))
            .collect();
        let at = items.iter().position(|i| *i == self.selected).unwrap_or(0);
        if let Some(next) = items.get(
            at.saturating_add_signed(delta)
                .min(items.len().saturating_sub(1)),
        ) {
            if *next != self.selected {
                self.selected = *next;
                self.reset_view();
            }
        }
    }

    fn page(&self) -> isize {
        self.visible.get().saturating_sub(1).max(1) as isize
    }

    fn cycle_category(&mut self) {
        let all = self.catalog.categories();
        self.category = match &self.category {
            None => all.first().cloned(),
            Some(c) => all
                .iter()
                .position(|x| x.eq_ignore_ascii_case(c))
                .and_then(|i| all.get(i + 1).cloned()),
        };
        self.rebuild();
    }

    fn clear_filters(&mut self) {
        self.filter.clear();
        self.category = None;
        self.rebuild();
    }

    fn roll_request(kind: ItemKind, id: &str) -> Action {
        let id = id.to_string();
        Action::Roll(match kind {
            ItemKind::Table => RollRequest::Table { id },
            ItemKind::Generator => RollRequest::Generator { id },
        })
    }

    fn toggle_entries(&mut self) -> Action {
        match self.current() {
            Some(i) if i.kind == ItemKind::Table => {
                self.view = if self.view == View::Entries {
                    View::Detail
                } else {
                    View::Entries
                };
                self.entries_scroll = 0;
                Action::None
            }
            _ => Action::Toast(Toast::info("entries view is for tables")),
        }
    }

    fn scroll_entries(&mut self, delta: isize) {
        let len = self
            .current()
            .and_then(|i| self.catalog.table(&i.id))
            .map_or(0, |t| t.entries.len());
        self.entries_scroll = self
            .entries_scroll
            .saturating_add_signed(delta)
            .min(len.saturating_sub(1));
    }

    fn focus(&mut self, target: &LibraryTarget) {
        self.filter.clear();
        match target {
            LibraryTarget::Item { kind, id } => {
                self.category = None;
                self.rebuild();
                if let Some(at) = self.position(*kind, id) {
                    self.selected = at;
                }
            }
            LibraryTarget::Category(c) => {
                let all = self.catalog.categories();
                self.category = Some(
                    all.iter()
                        .find(|x| x.eq_ignore_ascii_case(c))
                        .cloned()
                        .unwrap_or_else(|| c.clone()),
                );
                self.rebuild();
                self.selected = self.first_item().unwrap_or(0);
            }
        }
        self.reset_view();
    }

    fn filter_key(&mut self, key: KeyEvent) -> Action {
        match key.code {
            KeyCode::Enter => self.filtering = false,
            KeyCode::Esc => {
                self.filtering = false;
                self.filter.clear();
                self.rebuild();
            }
            _ => {
                if self.filter.handle_key(key) {
                    self.rebuild();
                }
            }
        }
        Action::None
    }

    fn list_lines(&self, width: usize) -> Vec<Line<'static>> {
        let rows = self.visible.get();
        let mut top = self.top.get();
        if self.selected < top {
            top = self.selected;
        } else if self.selected >= top + rows {
            top = self.selected + 1 - rows;
        }
        self.top.set(top);
        self.rows
            .iter()
            .enumerate()
            .skip(top)
            .take(rows)
            .map(|(i, row)| match row {
                Row::Header(c) => Line::styled(clip(c, width), theme::bold()),
                Row::Item(item) => {
                    let marker = match item.kind {
                        ItemKind::Table => 'T',
                        ItemKind::Generator => 'G',
                    };
                    let text = clip(&format!("{marker} {}", item.name), width);
                    if i == self.selected {
                        Line::styled(text, theme::active_tab())
                    } else {
                        Line::from(text)
                    }
                }
            })
            .collect()
    }

    fn filter_line(&self) -> Line<'static> {
        let (before, after) = self.filter.split_at_cursor();
        let mut spans = vec![Span::raw("Filter: "), Span::raw(before.to_string())];
        if self.filtering {
            let mut rest = after.chars();
            let at = rest.next().map_or(" ".to_string(), String::from);
            spans.push(Span::styled(at, theme::active_tab()));
            spans.push(Span::raw(rest.collect::<String>()));
        } else {
            spans.push(Span::raw(after.to_string()));
        }
        if let Some(c) = &self.category {
            spans.push(Span::raw(format!("  [{c}]")));
        }
        Line::from(spans)
    }

    fn detail_lines(&self) -> Vec<Line<'static>> {
        let Some(item) = self.current() else {
            return vec![Line::styled("No items match.", theme::dim())];
        };
        let mut lines = vec![Line::styled(item.name.clone(), theme::bold())];
        lines.push(Line::styled(
            format!("{} \u{b7} {}", item.id, item.category),
            theme::dim(),
        ));
        if let Some(d) = &item.description {
            lines.push(Line::from(d.clone()));
        }
        match item.kind {
            ItemKind::Table => {
                if let Some(t) = self.catalog.table(&item.id) {
                    lines.push(Line::from(format!("dice: {}", t.dice)));
                }
            }
            ItemKind::Generator => {
                if let Some(g) = self.catalog.generator(&item.id) {
                    lines.push(Line::styled("Slots:", theme::bold()));
                    for s in &g.slots {
                        let source = match &s.source {
                            SlotSource::Table(id) => format!("table {id}"),
                            SlotSource::Generator(id) => format!("generator {id}"),
                            SlotSource::Dice(d) => format!("dice {d}"),
                        };
                        lines.push(Line::from(format!("  {}: {source}", s.key)));
                    }
                }
            }
        }
        lines.push(Line::from(""));
        match &self.last {
            Some(last) if last.kind == item.kind && last.id == item.id => {
                lines.extend(Self::result_lines(&last.result));
            }
            _ => lines.push(Line::styled("Enter to roll.", theme::dim())),
        }
        lines
    }

    fn result_lines(result: &RollResult) -> Vec<Line<'static>> {
        match result {
            RollResult::Table(r) => vec![
                Line::styled("Result:", theme::bold()),
                Line::from(r.dice.to_string()),
                Line::from(r.text.clone()),
            ],
            RollResult::Generator(g) => {
                let mut lines = vec![Line::styled("Result:", theme::bold())];
                lines.extend(result_block::lines(g));
                lines
            }
            _ => Vec::new(),
        }
    }

    fn entry_lines(&self) -> Vec<Line<'static>> {
        let Some(table) = self.current().and_then(|i| self.catalog.table(&i.id)) else {
            return Vec::new();
        };
        let mut lines = vec![Line::styled(
            format!("Entries ({})", table.entries.len()),
            theme::bold(),
        )];
        for (n, e) in table.entries.iter().enumerate() {
            let label = match table.form {
                TableForm::List => (n + 1).to_string(),
                TableForm::Ranged if e.lo == e.hi => e.lo.to_string(),
                TableForm::Ranged => format!("{}-{}", e.lo, e.hi),
            };
            lines.push(Line::from(format!("{label:>7}  {}", e.raw)));
        }
        lines
    }
}

impl Tab for LibraryTab {
    fn id(&self) -> TabId {
        TabId::Library
    }

    fn title(&self) -> &'static str {
        "Library"
    }

    fn update(&mut self, key: KeyEvent) -> Action {
        if self.filtering {
            return self.filter_key(key);
        }
        let in_entries = self.view == View::Entries;
        match key.code {
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Char('c') => self.cycle_category(),
            KeyCode::Char('v') => return self.toggle_entries(),
            KeyCode::Esc => self.clear_filters(),
            KeyCode::Up => self.move_by(-1),
            KeyCode::Down => self.move_by(1),
            KeyCode::PageUp if in_entries => self.scroll_entries(-self.page()),
            KeyCode::PageDown if in_entries => self.scroll_entries(self.page()),
            KeyCode::PageUp => self.move_by(-self.page()),
            KeyCode::PageDown => self.move_by(self.page()),
            KeyCode::Home => self.move_by(isize::MIN / 2),
            KeyCode::End => self.move_by(isize::MAX / 2),
            KeyCode::Enter => {
                if let Some(i) = self.current() {
                    return Self::roll_request(i.kind, &i.id);
                }
            }
            KeyCode::Char('r') => {
                return match &self.last {
                    Some(l) => Self::roll_request(l.kind, &l.id),
                    None => Action::Toast(Toast::info("nothing rolled yet")),
                };
            }
            _ => {}
        }
        Action::None
    }

    fn render(&self, frame: &mut Frame, area: Rect) {
        let outer = Block::default().borders(Borders::ALL).title("Library");
        let inner = outer.inner(area);
        frame.render_widget(outer, area);
        let list_w = (inner.width * 2 / 5).max(MIN_LIST_WIDTH).min(inner.width);
        let [left, right] =
            Layout::horizontal([Constraint::Length(list_w), Constraint::Min(0)]).areas(inner);
        let [list, filter] =
            Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(left);
        self.visible.set(usize::from(list.height).max(1));
        frame.render_widget(
            Paragraph::new(self.list_lines(usize::from(list.width))),
            list,
        );
        frame.render_widget(Paragraph::new(self.filter_line()), filter);

        let showing_entries = self.view == View::Entries;
        let lines = if showing_entries {
            self.entry_lines()
        } else {
            self.detail_lines()
        };
        let scroll = if showing_entries {
            self.entries_scroll
        } else {
            0
        };
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((u16::try_from(scroll).unwrap_or(u16::MAX), 0))
                .block(Block::default().borders(Borders::LEFT)),
            right,
        );
    }

    fn captures_input(&self) -> bool {
        self.filtering
    }

    fn on_event(&mut self, ev: &TabEvent) {
        match ev {
            TabEvent::Rolled(rolled) => {
                let (kind, id) = match &rolled.result {
                    RollResult::Table(r) => (ItemKind::Table, r.table_id.clone()),
                    RollResult::Generator(g) => (ItemKind::Generator, g.id.clone()),
                    _ => return,
                };
                self.last = Some(Last {
                    kind,
                    id,
                    result: rolled.result.clone(),
                });
            }
            TabEvent::Focus(target) => self.focus(target),
            _ => {}
        }
    }

    fn help(&self) -> &'static [(&'static str, &'static str)] {
        &[
            ("/", "filter"),
            ("c", "category"),
            ("Enter", "roll"),
            ("r", "repeat"),
            ("v", "entries"),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testkit::{engine, fixture_catalog};
    use crossterm::event::KeyModifiers;
    use oraculum_core::catalog::Source;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ch(c: char) -> KeyEvent {
        key(KeyCode::Char(c))
    }

    fn tab() -> LibraryTab {
        LibraryTab::new(Rc::new(fixture_catalog().0))
    }

    fn type_str(tab: &mut LibraryTab, s: &str) {
        for c in s.chars() {
            tab.update(ch(c));
        }
    }

    fn ids(tab: &LibraryTab) -> Vec<String> {
        tab.rows
            .iter()
            .filter_map(|r| match r {
                Row::Item(i) => Some(i.id.clone()),
                Row::Header(_) => None,
            })
            .collect()
    }

    fn screen(tab: &LibraryTab, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| tab.render(f, f.area())).unwrap();
        let buf = term.backend().buffer().clone();
        (0..h)
            .map(|y| (0..w).map(|x| buf[(x, y)].symbol()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn current_id(tab: &LibraryTab) -> String {
        tab.current().map(|i| i.id.clone()).unwrap_or_default()
    }

    #[test]
    fn list_groups_by_category_with_kind_markers_and_skips_headers() {
        let mut t = tab();
        let s = screen(&t, 80, 20);
        for want in ["Alpha", "Beta", "Oracle", "T colors", "G pair"] {
            assert!(s.contains(want), "{want}\n{s}");
        }
        assert_eq!(current_id(&t), "colors");
        t.update(key(KeyCode::Down));
        assert_eq!(current_id(&t), "loot");
        t.update(key(KeyCode::Down));
        assert_eq!(current_id(&t), "inner", "the Beta header is skipped");
        t.update(key(KeyCode::Up));
        assert_eq!(current_id(&t), "loot");
    }

    #[test]
    fn filter_matches_id_name_category_and_tag_live() {
        let mut t = tab();
        t.update(ch('/'));
        assert!(t.captures_input());
        type_str(&mut t, "pai");
        assert_eq!(ids(&t), ["pair"], "id");
        for _ in 0..3 {
            t.update(key(KeyCode::Backspace));
        }
        type_str(&mut t, "ORACLE");
        assert_eq!(ids(&t), ["event"], "category, any case");
        t.update(key(KeyCode::Esc));
        assert!(ids(&t).len() > 3, "Esc clears the filter");
        t.update(ch('/'));
        type_str(&mut t, "hue");
        assert_eq!(ids(&t), ["colors"], "tag");
        t.update(key(KeyCode::Enter));
        assert!(!t.captures_input());
        assert_eq!(ids(&t), ["colors"], "Enter keeps the filter");
    }

    #[test]
    fn keys_are_text_while_filtering() {
        let mut t = tab();
        t.update(ch('/'));
        type_str(&mut t, "cvr");
        assert_eq!(t.filter.text(), "cvr");
        assert_eq!(t.category, None);
        assert_eq!(t.view, View::Detail);
    }

    #[test]
    fn esc_in_browse_clears_the_text_and_the_category() {
        let mut t = tab();
        t.update(ch('c'));
        t.update(ch('/'));
        type_str(&mut t, "zzz");
        t.update(key(KeyCode::Enter));
        assert!(ids(&t).is_empty());
        t.update(key(KeyCode::Esc));
        assert_eq!(t.category, None);
        assert_eq!(t.filter.text(), "");
        assert_eq!(ids(&t).len(), 7);
    }

    #[test]
    fn c_cycles_all_then_alphabetical_then_all() {
        let mut t = tab();
        let mut seen = Vec::new();
        for _ in 0..4 {
            t.update(ch('c'));
            seen.push(t.category.clone());
        }
        assert_eq!(
            seen,
            [
                Some("Alpha".to_string()),
                Some("Beta".to_string()),
                Some("Oracle".to_string()),
                None
            ]
        );
        assert!(screen(&t, 80, 20).contains("Filter:"));
    }

    #[test]
    fn the_selection_stays_on_the_same_item_when_it_is_still_listed() {
        let mut t = tab();
        t.update(ch('c')); // Alpha
        t.update(ch('c')); // Beta: first is inner
        t.update(key(KeyCode::Down)); // opt? check order
        let chosen = current_id(&t);
        t.update(ch('c')); // Oracle: chosen no longer listed
        assert_eq!(current_id(&t), "event");
        t.update(ch('c')); // all
        t.update(ch('/'));
        type_str(&mut t, "ev");
        assert_eq!(current_id(&t), "event");
        assert_ne!(chosen, "event");
    }

    #[test]
    fn v_shows_entries_with_ranges_for_tables_and_toasts_for_generators() {
        let mut t = tab();
        t.update(key(KeyCode::Down)); // loot (ranged)
        t.update(ch('v'));
        let s = screen(&t, 80, 20);
        assert!(s.contains("Entries (3)"), "{s}");
        assert!(s.contains("1-2  copper"), "{s}");
        assert!(s.contains("6  gold"), "{s}");
        t.update(ch('v'));
        assert!(screen(&t, 80, 20).contains("dice: 1d6"));
        // list table: numbered
        t.update(key(KeyCode::Up));
        t.update(ch('v'));
        assert!(screen(&t, 80, 20).contains("2  green"));
        // generator: toast, no view change
        t.update(ch('v'));
        t.update(ch('c'));
        t.update(ch('c'));
        assert_eq!(
            t.update(ch('v')),
            Action::Toast(Toast::info("entries view is for tables"))
        );
        assert_eq!(t.view, View::Detail);
    }

    #[test]
    fn enter_rolls_the_selected_item_and_r_repeats_the_displayed_one() {
        let mut t = tab();
        assert_eq!(
            t.update(key(KeyCode::Enter)),
            Action::Roll(RollRequest::Table {
                id: "colors".into()
            })
        );
        assert_eq!(
            t.update(ch('r')),
            Action::Toast(Toast::info("nothing rolled yet"))
        );
        let mut e = engine(&[2, 1, 1, 1]);
        let rolled = e.roll_table("colors").unwrap();
        t.on_event(&TabEvent::Rolled(Box::new(rolled)));
        assert!(screen(&t, 80, 20).contains("green"));
        // move away; r still repeats the displayed result's item
        t.update(key(KeyCode::Down));
        assert_eq!(
            t.update(ch('r')),
            Action::Roll(RollRequest::Table {
                id: "colors".into()
            })
        );
        // Enter on a generator
        t.update(key(KeyCode::Down));
        assert_eq!(
            t.update(key(KeyCode::Enter)),
            Action::Roll(RollRequest::Generator { id: "inner".into() })
        );
    }

    #[test]
    fn generator_results_show_the_text_and_slot_overview() {
        let mut t = tab();
        t.focus(&LibraryTarget::Item {
            kind: ItemKind::Generator,
            id: "pair".into(),
        });
        let mut e = engine(&[2, 3, 4]);
        let rolled = e.generate("pair").unwrap();
        t.on_event(&TabEvent::Rolled(Box::new(rolled)));
        let s = screen(&t, 100, 24);
        assert!(s.contains("green and gamma (4)"), "{s}");
        assert!(s.contains("b: generator inner"), "{s}");
    }

    #[test]
    fn focus_category_filters_and_selects_the_first_item() {
        let mut t = tab();
        t.update(ch('/'));
        type_str(&mut t, "col");
        t.update(key(KeyCode::Enter));
        t.on_event(&TabEvent::Focus(LibraryTarget::Category("Oracle".into())));
        assert_eq!(t.category.as_deref(), Some("Oracle"));
        assert_eq!(t.filter.text(), "");
        assert_eq!(ids(&t), ["event"]);
        assert_eq!(current_id(&t), "event");
    }

    #[test]
    fn focus_item_clears_filters_and_selects_it() {
        let mut t = tab();
        t.update(ch('c'));
        t.on_event(&TabEvent::Focus(LibraryTarget::Item {
            kind: ItemKind::Generator,
            id: "opt".into(),
        }));
        assert_eq!(t.category, None);
        assert_eq!(current_id(&t), "opt");
    }

    #[test]
    fn user_added_items_appear_in_the_list() {
        let user = vec![Source::table(
            "user",
            "mine",
            "name = \"Mine\"\ncategory = \"Mine\"\nitems = [\"a\", \"b\"]",
        )];
        let (catalog, _) = Catalog::build(&[], &user);
        let t = LibraryTab::new(Rc::new(catalog));
        let s = screen(&t, 80, 20);
        assert!(s.contains("T Mine"), "{s}");
    }

    #[test]
    fn an_empty_library_renders_and_ignores_keys() {
        let (catalog, _) = Catalog::build(&[], &[]);
        let mut t = LibraryTab::new(Rc::new(catalog));
        for code in [
            KeyCode::Down,
            KeyCode::PageUp,
            KeyCode::Enter,
            KeyCode::Home,
        ] {
            assert_eq!(t.update(key(code)), Action::None);
        }
        assert!(screen(&t, 60, 20).contains("No items match."));
    }

    #[test]
    fn renders_at_the_minimum_size() {
        let t = tab();
        let s = screen(&t, 60, 18);
        assert!(s.contains("T colors") && s.contains("Filter:"), "{s}");
    }
}
