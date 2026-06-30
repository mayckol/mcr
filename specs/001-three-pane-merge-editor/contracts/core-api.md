# Contract: `mcr-core` Public API

The Rust crate exposing all merge logic. The Tauri shell and the future mergetool entrypoint both call this; no merge logic exists outside it.

## `MergeSession::open(input) -> MergeSession`

**input** (`OpenInput`):

| Field | Type | Required | Notes |
|-------|------|----------|-------|
| `local` | string | yes | Left version |
| `ancestor` | string | yes | Common base |
| `incoming` | string | yes | Right version |
| `whitespace_mode` | enum | no | default `none` |

**Returns**: a fully aligned `MergeSession` (hunks, alignment, word spans, status). Read-only inputs; `result` initialized from a clean merge of non-conflicting regions, conflicts left `unresolved`.

## `session.apply(hunk_id, from) -> Delta`

Inserts `from`'s (`local`|`incoming`) content for the hunk into `result`. Pushes an Operation. **Returns** a `Delta` (changed hunks + new `result` slice + new status). Error if `hunk_id` unknown or `from` has no content for that hunk.

## `session.revert(hunk_id) -> Delta`

Undoes the hunk's applied content, restoring its `unresolved`/pre-edit state via the Operation log. **Returns** `Delta`. No-op error if the hunk was never applied/edited.

## `session.apply_non_conflicting(from) -> Delta`

`from` ∈ `local` | `incoming` | `both`. Applies every non-`conflicting` hunk from that side in one transaction (FR-009). Skips `conflicting` hunks. **Returns** aggregate `Delta`.

## `session.edit_result(range, text) -> Delta`

Records a manual center-pane edit; recomputes affected hunk states (→ `manually_edited`) and status (FR-012). **Returns** `Delta`.

## `session.undo() -> Delta` / `session.redo() -> Delta`

Pops/replays the inverse/forward Operation (FR-010, SC-005). **Returns** `Delta`.

## `session.navigate(direction, from_hunk?) -> HunkId`

`direction` ∈ `next` | `prev`. Returns the target `HunkId` so the UI can reveal it in all three panes (FR-016). Wraps or clamps at ends (clamp default).

## `session.status() -> ResolutionStatus`

Returns the live `ResolutionStatus` (FR-013).

## Invariants

- Inputs `local`/`ancestor`/`incoming` never mutate (FR-015).
- Every mutating call returns a `Delta`; the UI never recomputes hunks/alignment.
- `apply(h, f)` then `revert(h)` restores the exact prior `result` (round-trip, Principle IV / SC-005).
- Operations are scoped to one hunk's `result_range` and never touch other regions.
