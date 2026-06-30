mod fixtures;

use mcr_core::{Category, MergeSession, WhitespaceMode};

fn open(f: (&str, &str, &str)) -> MergeSession {
    MergeSession::open("t", f.0, f.1, f.2, WhitespaceMode::None)
}

#[test]
fn mixed_detects_two_non_conflicting_changes() {
    let m = open(fixtures::mixed()).to_model();
    assert_eq!(m.hunks.len(), 2);
    assert!(m.hunks.iter().all(|h| h.category != Category::Conflicting));
    // Non-conflicting changes auto-applied -> result already reflects both sides.
    assert!(m.panes.result.contains(&"alpha-LOCAL".to_string()));
    assert!(m.panes.result.contains(&"delta-INCOMING".to_string()));
    assert!(m.status.fully_resolved);
    assert_eq!(m.status.remaining_conflicts, 0);
}

#[test]
fn every_alignment_row_indexes_into_its_pane() {
    let m = open(fixtures::mixed()).to_model();
    for row in &m.alignment {
        if let Some(i) = row.local {
            assert!(i < m.panes.local.len());
        }
        if let Some(i) = row.result {
            assert!(i < m.panes.result.len());
        }
        if let Some(i) = row.incoming {
            assert!(i < m.panes.incoming.len());
        }
    }
}

#[test]
fn filler_keeps_panes_aligned_on_differing_line_counts() {
    // local has fewer lines than incoming in the changed block.
    let m = open(fixtures::delete_vs_modify()).to_model();
    // There is at least one row where one side is filler (None) inside a hunk.
    let has_filler = m
        .alignment
        .iter()
        .any(|r| r.hunk.is_some() && (r.local.is_none() || r.incoming.is_none()));
    assert!(
        has_filler,
        "expected filler rows in a delete-vs-modify block"
    );
    // Result line count equals number of rows whose result is Some.
    let result_rows = m.alignment.iter().filter(|r| r.result.is_some()).count();
    assert_eq!(result_rows, m.panes.result.len());
}

#[test]
fn result_ranges_are_contiguous_and_ordered() {
    let m = open(fixtures::mixed()).to_model();
    let mut prev_end = 0;
    for h in &m.hunks {
        assert!(h.result_range.start >= prev_end);
        assert!(h.result_range.end >= h.result_range.start);
        prev_end = h.result_range.end;
    }
}
