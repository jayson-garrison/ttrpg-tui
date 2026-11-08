//! A generator result as styled lines: text that came from a slot is bold and
//! underlined, so the highlight never changes the words (D-79).

use oraculum_core::generator::GeneratorResult;
use ratatui::text::{Line, Span};

use crate::tui::theme;

/// One `Line` per line of the rendered text. Segment text may span lines.
pub fn lines(result: &GeneratorResult) -> Vec<Line<'static>> {
    let mut out: Vec<Vec<Span<'static>>> = vec![Vec::new()];
    for seg in &result.segments {
        let mut parts = seg.text.split('\n');
        if let Some(first) = parts.next() {
            push(&mut out, first, seg.slot.is_some());
        }
        for part in parts {
            out.push(Vec::new());
            push(&mut out, part, seg.slot.is_some());
        }
    }
    out.into_iter().map(Line::from).collect()
}

fn push(out: &mut [Vec<Span<'static>>], text: &str, slot: bool) {
    if text.is_empty() {
        return;
    }
    let span = if slot {
        Span::styled(text.to_string(), theme::slot())
    } else {
        Span::raw(text.to_string())
    };
    if let Some(line) = out.last_mut() {
        line.push(span);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::testkit::engine;
    use oraculum_core::engine::RollResult;

    fn pair() -> GeneratorResult {
        // colors -> 2 (green); inner -> words 3 (gamma); 1d6 -> 4.
        let mut e = engine(&[2, 3, 4]);
        match e.generate("pair").map(|r| r.result) {
            Ok(RollResult::Generator(g)) => g,
            other => unreachable!("{other:?}"),
        }
    }

    #[test]
    fn slot_text_is_styled_and_the_words_are_unchanged() {
        let g = pair();
        let lines = lines(&g);
        assert_eq!(lines.len(), 1);
        let plain: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert_eq!(plain, g.text);
        assert_eq!(plain, "green and gamma (4)");
        let styled: Vec<&str> = lines[0]
            .spans
            .iter()
            .filter(|s| s.style == theme::slot())
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(styled, ["green", "gamma", "4"]);
    }

    #[test]
    fn multi_line_results_become_multiple_lines() {
        // opt: tail chance 50 -> roll 1 keeps it; colors -> 3 (blue).
        let mut e = engine(&[1, 3]);
        let Ok(RollResult::Generator(g)) = e.generate("opt").map(|r| r.result) else {
            unreachable!("opt")
        };
        let rendered: Vec<String> = lines(&g)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rendered, g.text.split('\n').collect::<Vec<_>>());
        assert_eq!(rendered.len(), 2);
    }
}
