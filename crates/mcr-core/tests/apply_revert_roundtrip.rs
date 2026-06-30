mod fixtures;

use mcr_core::{Category, HunkState, MergeSession, Side, WhitespaceMode};

fn open(f: (&str, &str, &str)) -> MergeSession {
    MergeSession::open("t", f.0, f.1, f.2, WhitespaceMode::None)
}

/// Constitution Principle IV / SC-005: apply then revert restores the original.
#[test]
fn apply_then_revert_restores_original_result() {
    let mut s = open(fixtures::conflict());
    let before = s.to_model().panes.result;

    // Resolve the single conflict by applying local, then revert it.
    let conflict_id = s
        .to_model()
        .hunks
        .iter()
        .find(|h| h.category == Category::Conflicting)
        .unwrap()
        .id;

    s.apply(conflict_id, Side::Local);
    let applied = s.to_model();
    assert!(applied.panes.result.contains(&"two-LEFT".to_string()));
    assert_eq!(applied.status.remaining_conflicts, 0);

    let after = s.revert(conflict_id).panes.result;
    assert_eq!(before, after, "revert must restore the exact prior result");
}

#[test]
fn applying_conflict_left_then_right_switches_content() {
    let mut s = open(fixtures::conflict());
    let id = s.to_model().hunks[0].id;

    let left = s.apply(id, Side::Local).panes.result;
    assert!(left.contains(&"two-LEFT".to_string()));

    let right = s.apply(id, Side::Incoming).panes.result;
    assert!(right.contains(&"two-RIGHT".to_string()));
    assert!(!right.contains(&"two-LEFT".to_string()));
}

#[test]
fn adjacent_hunks_apply_revert_independently() {
    let mut s = open(fixtures::mixed());
    let model = s.to_model();
    assert_eq!(model.hunks.len(), 2);
    let (h0, h1) = (model.hunks[0].id, model.hunks[1].id);

    // Revert the first hunk; the second must stay applied.
    s.revert(h0);
    let m = s.to_model();
    assert!(matches!(
        m.hunks.iter().find(|h| h.id == h0).unwrap().state,
        HunkState::Unresolved
    ));
    assert!(matches!(
        m.hunks.iter().find(|h| h.id == h1).unwrap().state,
        HunkState::Applied { .. }
    ));
}

#[test]
fn undo_redo_restore_exact_state() {
    let mut s = open(fixtures::conflict());
    let id = s.to_model().hunks[0].id;
    let original = s.to_model().panes.result;

    s.apply(id, Side::Incoming);
    let applied = s.to_model().panes.result;
    assert_ne!(original, applied);

    s.undo();
    assert_eq!(s.to_model().panes.result, original);

    s.redo();
    assert_eq!(s.to_model().panes.result, applied);
}

#[test]
fn apply_non_conflicting_skips_conflicts() {
    // Build a fixture with a non-conflict and a conflict.
    let ancestor = "h\nkeep\nx";
    let local = "h\nkeep\nx-LEFT"; // local change on line 3
    let incoming = "h-INC\nkeep\nx-RIGHT"; // incoming change line1 (non-conf) + line3 (conflict)
    let mut s = MergeSession::open("t", local, ancestor, incoming, WhitespaceMode::None);

    // Start by reverting everything to a clean unresolved baseline for the conflict.
    let m = s.apply_non_conflicting(None);
    // The conflicting hunk remains unresolved.
    assert!(m.status.remaining_conflicts >= 1);
    assert!(m.hunks.iter().any(|h| h.category == Category::Conflicting));
}

#[test]
fn delete_vs_modify_pairs_correctly() {
    let mut s = open(fixtures::delete_vs_modify());
    let m = s.to_model();
    // There should be a conflicting region (local deletes b/c, incoming modifies b).
    assert!(m.hunks.iter().any(|h| h.category == Category::Conflicting));
    // Apply incoming -> keeps modified line; apply local -> drops them.
    let id = m
        .hunks
        .iter()
        .find(|h| h.category == Category::Conflicting)
        .unwrap()
        .id;
    let inc = s.apply(id, Side::Incoming).panes.result;
    assert!(inc.iter().any(|l| l.contains("b-MOD")));
    let loc = s.apply(id, Side::Local).panes.result;
    assert!(!loc.iter().any(|l| l.contains("b-MOD")));
}
