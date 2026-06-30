# Quickstart: Three-Pane Visual Merge Editor

Validation guide proving the editor works end to end. References `contracts/` and `data-model.md` rather than duplicating them.

## Prerequisites

- Rust 1.79+ (`rustup`), Node 20+, Tauri 2.x prerequisites for your OS.
- Workspace builds: `cargo build` (root) and `npm install` in `ui/`.

## Run the app

```bash
# from repo root
npm --prefix ui run dev      # frontend dev server
cargo tauri dev              # launches the Tauri shell against ui/
```

Provide a sample session via a dev fixture (local/ancestor/incoming strings) wired to `open_session`.

## Validation scenarios

### S1 — Three panes render and trace (User Story 1)

1. Open a fixture with ≥1 conflict and one non-conflicting change per side.
2. **Expect**: three panes (local / editable result / incoming); every hunk has a line band; every on-screen hunk has a connector to its `result_range`; filler keeps lines aligned.
3. Pass = matches FR-001..FR-005, SC-002.

### S2 — Apply / revert loop (User Story 2)

1. Apply a left hunk, then a right hunk → result patches immediately (`Delta.result_patch`).
2. Revert one → only that region returns to `unresolved`; others unchanged.
3. Use "apply all non-conflicting from left" → conflicts skipped.
4. Global undo → reverses the last op exactly.
5. Pass = FR-007..FR-010, FR-012, SC-001, SC-003 (<100 ms on ≤5k lines), SC-005.

### S3 — Precise binding follows lines (User Story 3)

1. Open a fixture with modified lines → word-level spans mark exact changed words inside the band.
2. Scroll any pane → all three scroll in sync; connectors re-anchor with no drift.
3. Resize the window → highlights/connectors realign.
4. Pass = FR-006, FR-011, SC-004, SC-006.

### S4 — Whitespace toggle (FR-014)

1. Toggle ignore-whitespace → `set_whitespace_mode` re-diffs; whitespace-only hunks disappear/reappear.

### S5 — Status & resolution (FR-013)

1. Resolve all conflicts → status shows `remaining_conflicts: 0`, `fully_resolved: true`.

## Automated checks (must pass in CI)

- `cargo test -p mcr-core` — alignment, word-diff, and the **apply→revert round-trip** test restoring original result (Principle IV).
- `npm --prefix ui test` — pane binding, decoration mapping, scroll-sync, connector anchoring under scroll/resize.
- Vendor-neutral lint: grep the build output/source for the inspiration vendor name → must be zero hits (FR-017, Principle VI).
