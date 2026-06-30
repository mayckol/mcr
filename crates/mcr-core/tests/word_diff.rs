mod fixtures;

use mcr_core::{Category, MergeSession, Pane, WhitespaceMode};

fn open(f: (&str, &str, &str)) -> MergeSession {
    MergeSession::open("t", f.0, f.1, f.2, WhitespaceMode::None)
}

#[test]
fn conflict_is_detected_with_word_spans() {
    let m = open(fixtures::conflict()).to_model();
    let conflicts: Vec<_> = m
        .hunks
        .iter()
        .filter(|h| h.category == Category::Conflicting)
        .collect();
    assert_eq!(conflicts.len(), 1);
    let h = conflicts[0];
    // "two-LEFT" vs "two-RIGHT": the differing suffix is marked on both panes.
    assert!(h.word_spans.iter().any(|s| s.pane == Pane::Local));
    assert!(h.word_spans.iter().any(|s| s.pane == Pane::Incoming));
}

#[test]
fn word_spans_target_exact_changed_columns() {
    // ancestor irrelevant; force a modified hunk on one side vs other.
    let f = ("foobar\n", "fooXXX\n", "fooBAZ\n");
    let m = open(f).to_model();
    let h = m
        .hunks
        .iter()
        .find(|h| !h.word_spans.is_empty())
        .expect("a hunk with word spans");
    // Shared prefix "foo" (cols 0..3) must be untouched.
    for s in &h.word_spans {
        assert!(s.start_col >= 3, "prefix should not be highlighted: {s:?}");
    }
}

#[test]
fn whitespace_ignore_all_hides_whitespace_only_change() {
    let f = ("a  b\nc", "a b\nc", "a b\nc"); // local differs only by spaces
    let none = MergeSession::open("t", f.0, f.1, f.2, WhitespaceMode::None)
        .to_model()
        .hunks
        .len();
    let ignore = MergeSession::open("t", f.0, f.1, f.2, WhitespaceMode::IgnoreAll)
        .to_model()
        .hunks
        .len();
    assert_eq!(none, 1);
    assert_eq!(ignore, 0);
}
