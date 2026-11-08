//! Text the CLI prints: warnings (D-60), unknown-id errors (D-61), the list.

use oraculum_core::catalog::{ItemKind, ItemSummary, LoadWarning, WarningKind};

/// `warning: <file>: <item>: <what is wrong> (<what happened>)`
pub fn format_warning(w: &LoadWarning) -> String {
    let outcome = match w.kind {
        _ if w.kept_builtin => "kept built-in",
        WarningKind::Replaced => "replaced the built-in",
        WarningKind::DuplicateUserId => "used the later file",
        WarningKind::UnusedSlot | WarningKind::IgnoredField => "kept",
        _ => "skipped",
    };
    // One warning per line, whatever the message holds.
    let message = w.message.split_whitespace().collect::<Vec<_>>().join(" ");
    match &w.item {
        Some((_, id)) => format!("warning: {}: {id}: {message} ({outcome})", w.origin),
        None => format!("warning: {}: {message} ({outcome})", w.origin),
    }
}

/// `warning: <journal>: line N: <reason> (skipped)`
pub fn format_skipped_line(path: &std::path::Path, line_no: usize, reason: &str) -> String {
    format!(
        "warning: {}: line {line_no}: {reason} (skipped)",
        path.display()
    )
}

/// Classic dynamic-programming edit distance over chars.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// Up to three ids close to `wanted`, nearest first.
pub fn suggestions<'a>(wanted: &str, ids: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let limit = (wanted.chars().count() / 3).max(2);
    let mut scored: Vec<(usize, &str)> = ids
        .map(|id| (edit_distance(wanted, id), id))
        .filter(|(d, _)| *d <= limit)
        .collect();
    scored.sort();
    scored.into_iter().take(3).map(|(_, id)| id).collect()
}

pub fn unknown_id_message(kind: ItemKind, id: &str, close: &[&str]) -> String {
    let noun = match kind {
        ItemKind::Table => "table",
        ItemKind::Generator => "generator",
    };
    if close.is_empty() {
        format!("no {noun} with the id {id:?}; run `oraculum list` to see what is available")
    } else {
        format!(
            "no {noun} with the id {id:?} (did you mean: {}?)",
            close.join(", ")
        )
    }
}

/// `{T|G}  {id}  {category}  {name}`, id and category padded to the widest.
pub fn format_list(items: &[&ItemSummary]) -> String {
    let id_w = items
        .iter()
        .map(|i| i.id.chars().count())
        .max()
        .unwrap_or(0);
    let cat_w = items
        .iter()
        .map(|i| i.category.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for i in items {
        let mark = match i.kind {
            ItemKind::Table => 'T',
            ItemKind::Generator => 'G',
        };
        out.push_str(&format!(
            "{mark}  {:<id_w$}  {:<cat_w$}  {}\n",
            i.id, i.category, i.name
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_distance_basics() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("abc", "abc"), 0);
        assert_eq!(edit_distance("alpa", "alpha"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("", "abc"), 3);
    }

    #[test]
    fn suggestions_are_close_sorted_and_capped() {
        let ids = ["alpha", "alpine", "alps", "alpaca", "zebra"];
        let got = suggestions("alpa", ids.into_iter());
        assert_eq!(got.len(), 3);
        assert_eq!(got[0], "alpha");
        assert!(!got.contains(&"zebra"));
        assert!(suggestions("qqqqqq", ids.into_iter()).is_empty());
    }
}
