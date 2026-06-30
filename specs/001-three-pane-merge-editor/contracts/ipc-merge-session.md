# Contract: Tauri IPC — Merge Session

Typed commands the TS frontend invokes; thin wrappers over `mcr-core` (`contracts/core-api.md`). Payloads are `serde_json`. The UI applies returned deltas incrementally; it never derives merge state.

## Commands

| Command | Request | Response | Maps to |
|---------|---------|----------|---------|
| `open_session` | `{ local, ancestor, incoming, whitespace_mode? }` | `SessionModel` (full) | `MergeSession::open` |
| `apply_change` | `{ session_id, hunk_id, from }` | `Delta` | `session.apply` |
| `revert_change` | `{ session_id, hunk_id }` | `Delta` | `session.revert` |
| `apply_non_conflicting` | `{ session_id, from }` | `Delta` | `session.apply_non_conflicting` |
| `edit_result` | `{ session_id, range, text }` | `Delta` | `session.edit_result` |
| `undo` / `redo` | `{ session_id }` | `Delta` | `session.undo/redo` |
| `navigate` | `{ session_id, direction, from_hunk? }` | `{ hunk_id }` | `session.navigate` |
| `set_whitespace_mode` | `{ session_id, mode }` | `SessionModel` (full re-diff) | re-open with mode |

## `SessionModel` (open / re-diff response)

```json
{
  "session_id": "string",
  "panes": { "local": ["line", "..."], "result": ["..."], "incoming": ["..."] },
  "alignment": { "local_rows": [{"result_row": 0, "kind": "line|filler", "src": 0}], "incoming_rows": [] },
  "hunks": [{
    "id": "h1", "origin": "local|incoming|both",
    "category": "added|removed|modified|conflicting",
    "local_range": {"start": 0, "end": 0}, "incoming_range": null,
    "result_range": {"start": 0, "end": 0},
    "word_spans": [{"pane": "local", "row": 0, "start_col": 0, "end_col": 0}],
    "state": "unresolved"
  }],
  "status": { "total_hunks": 0, "remaining_conflicts": 0, "fully_resolved": false }
}
```

## `Delta` (mutating-command response)

```json
{
  "changed_hunks": [{ "id": "h1", "state": "applied", "from": "local", "result_range": {"start": 4, "end": 7} }],
  "result_patch": { "range": {"start": 4, "end": 5}, "lines": ["..."] },
  "status": { "total_hunks": 0, "remaining_conflicts": 0, "fully_resolved": true }
}
```

## UI rendering contract

- Three CodeMirror instances bind to `panes.local` / `panes.result` / `panes.incoming`; result editable, sides read-only.
- Line bands: `Decoration.line` per hunk `category`; word spans: `Decoration.mark` from `word_spans`.
- Filler rows from `alignment` render as blank gutter space to keep panes aligned (FR-005).
- Connectors: one SVG path per `{hunk, side}` from side `result_range`↔side range; anchored to live CodeMirror line geometry, re-projected on scroll/resize, culled off-screen (R3, SC-004, SC-007).
- After a `Delta`: patch the result document with `result_patch`, update only `changed_hunks` decorations/connectors, update status display.
- No command name, label, color token, or asset names or implies the inspiring IDE (FR-017).
