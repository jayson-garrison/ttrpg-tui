//! Single-line text input (D-34): no third-party input crate. Wide-glyph
//! cursor alignment is a known limitation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineInput {
    text: String,
    /// Char index, `0..=text.chars().count()`.
    cursor: usize,
}

impl LineInput {
    pub fn new() -> LineInput {
        LineInput::default()
    }

    pub fn with_text(text: &str) -> LineInput {
        LineInput {
            text: text.to_string(),
            cursor: text.chars().count(),
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.text.clear();
        self.cursor = 0;
    }

    /// Text before and after the cursor, for drawing it.
    pub fn split_at_cursor(&self) -> (&str, &str) {
        let at = self.byte_index(self.cursor);
        self.text.split_at(at)
    }

    fn len(&self) -> usize {
        self.text.chars().count()
    }

    fn byte_index(&self, chars: usize) -> usize {
        self.text
            .char_indices()
            .nth(chars)
            .map_or(self.text.len(), |(i, _)| i)
    }

    /// Applies an editing key. Returns true when the key was an editing key
    /// (so the caller knows the input consumed it).
    pub fn handle_key(&mut self, key: KeyEvent) -> bool {
        let plain = !key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT);
        match key.code {
            KeyCode::Char(c) if plain => {
                let at = self.byte_index(self.cursor);
                self.text.insert(at, c);
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let at = self.byte_index(self.cursor - 1);
                self.text.remove(at);
                self.cursor -= 1;
            }
            KeyCode::Delete if self.cursor < self.len() => {
                let at = self.byte_index(self.cursor);
                self.text.remove(at);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(self.len()),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.len(),
            KeyCode::Backspace | KeyCode::Delete => {}
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_str(input: &mut LineInput, s: &str) {
        for c in s.chars() {
            input.handle_key(key(KeyCode::Char(c)));
        }
    }

    #[test]
    fn typing_appends_and_moves_the_cursor() {
        let mut i = LineInput::new();
        type_str(&mut i, "2d6");
        assert_eq!(i.text(), "2d6");
        assert_eq!(i.cursor(), 3);
    }

    #[test]
    fn insert_in_the_middle_and_edit_keys() {
        let mut i = LineInput::with_text("ad");
        i.handle_key(key(KeyCode::Left));
        type_str(&mut i, "bc");
        assert_eq!(i.text(), "abcd");
        assert_eq!(i.split_at_cursor(), ("abc", "d"));
        i.handle_key(key(KeyCode::Backspace));
        assert_eq!(i.text(), "abd");
        i.handle_key(key(KeyCode::Delete));
        assert_eq!(i.text(), "ab");
        i.handle_key(key(KeyCode::Home));
        assert_eq!(i.cursor(), 0);
        i.handle_key(key(KeyCode::Backspace));
        assert_eq!(i.text(), "ab", "backspace at the start does nothing");
        i.handle_key(key(KeyCode::End));
        assert_eq!(i.cursor(), 2);
        i.handle_key(key(KeyCode::Delete));
        assert_eq!(i.text(), "ab", "delete at the end does nothing");
    }

    #[test]
    fn cursor_is_clamped_and_multibyte_safe() {
        let mut i = LineInput::with_text("h\u{e9}llo");
        i.handle_key(key(KeyCode::Home));
        i.handle_key(key(KeyCode::Right));
        i.handle_key(key(KeyCode::Delete));
        assert_eq!(i.text(), "hllo");
        for _ in 0..10 {
            i.handle_key(key(KeyCode::Right));
        }
        assert_eq!(i.cursor(), 4);
        for _ in 0..10 {
            i.handle_key(key(KeyCode::Left));
        }
        assert_eq!(i.cursor(), 0);
    }

    #[test]
    fn control_chords_and_other_keys_are_not_text() {
        let mut i = LineInput::new();
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert!(!i.handle_key(ctrl_c));
        assert!(!i.handle_key(key(KeyCode::Enter)));
        assert!(!i.handle_key(key(KeyCode::Esc)));
        assert_eq!(i.text(), "");
        i.clear();
        assert_eq!(i.cursor(), 0);
    }
}
