use mcr_core::{MergeSession, Side, WhitespaceMode};
use proptest::prelude::*;

fn session() -> MergeSession {
    let ancestor = "l1\nl2\nl3\nl4\nl5";
    let local = "l1-L\nl2\nl3-L\nl4\nl5";
    let incoming = "l1\nl2-I\nl3-I\nl4\nl5-I";
    MergeSession::open("p", local, ancestor, incoming, WhitespaceMode::None)
}

proptest! {
    /// A random sequence of apply/revert followed by undoing every operation
    /// must restore the exact original result (FR-010 / SC-005).
    #[test]
    fn undo_all_restores_original(seq in proptest::collection::vec((0usize..4, any::<bool>()), 0..40)) {
        let mut s = session();
        let original = s.to_model().panes.result.clone();
        let hunk_ids: Vec<usize> = s.to_model().hunks.iter().map(|h| h.id).collect();
        if hunk_ids.is_empty() { return Ok(()); }

        let mut ops = 0usize;
        for (pick, apply_local) in &seq {
            let id = hunk_ids[pick % hunk_ids.len()];
            if *apply_local {
                s.apply(id, Side::Local);
            } else {
                s.revert(id);
            }
            ops += 1;
        }

        // Undo more times than we applied; extra undos are no-ops.
        for _ in 0..(ops + 5) {
            s.undo();
        }
        prop_assert_eq!(s.to_model().panes.result, original);
    }
}
