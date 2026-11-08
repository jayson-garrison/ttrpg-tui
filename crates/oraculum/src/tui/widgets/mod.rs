//! Small stateless drawing helpers shared by the tabs.

pub mod help;
pub mod status_line;
pub mod tab_bar;
pub mod too_small;

/// Fit `text` in `width` columns, ending with `…` when it was cut.
pub fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    let keep = width.saturating_sub(1);
    let mut out: String = text.chars().take(keep).collect();
    if width > 0 {
        out.push('\u{2026}');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::clip;

    #[test]
    fn clip_cuts_with_an_ellipsis_and_never_panics() {
        assert_eq!(clip("hello", 10), "hello");
        assert_eq!(clip("hello", 5), "hello");
        assert_eq!(clip("hello world", 6), "hello\u{2026}");
        assert_eq!(clip("hello", 0), "");
        assert_eq!(clip("h\u{e9}llo wor", 4), "h\u{e9}l\u{2026}");
    }
}
