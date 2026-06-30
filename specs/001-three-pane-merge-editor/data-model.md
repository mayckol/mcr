# Phase 1 Data Model: Three-Pane Visual Merge Editor

All types live in `mcr-core` and are serialized to the UI. The UI treats them as read-only view state plus intent dispatch; it never derives merge state itself.

## MergeSession

The whole file being merged.

| Field | Type | Notes |
|-------|------|-------|
| `id` | SessionId | Opaque handle returned by `open_session` |
| `local` | PaneText | Left side (read-only) |
| `ancestor` | PaneText | Common base; not shown as a pane but drives conflict detection |
| `incoming` | PaneText | Right side (read-only) |
| `result` | PaneText | Editable center; the artifact eventually written out |
| `hunks` | ChangeRegion[] | Ordered top→bottom |
| `alignment` | Alignment | Per-pane row maps + filler |
| `status` | ResolutionStatus | Live overall state |
| `whitespace_mode` | WhitespaceMode | `none` \| `ignore_trailing` \| `ignore_all` (default `none`) |
| `undo` | OperationLog | Reversible history |

**Rules**: `local`/`ancestor`/`incoming` immutable for the session lifetime (FR-002, FR-015). `result` mutated only via Operations or tracked manual edits.

## ChangeRegion (Hunk)

A contiguous differing block.

| Field | Type | Notes |
|-------|------|-------|
| `id` | HunkId | Stable for the session |
| `origin` | Origin | `local` \| `incoming` \| `both` |
| `category` | Category | `added` \| `removed` \| `modified` \| `conflicting` |
| `local_range` | LineRange? | Lines in local (None if side absent) |
| `incoming_range` | LineRange? | Lines in incoming |
| `result_range` | LineRange | Span in the result spine (FR-005 anchor) |
| `word_spans` | IntraLineSpan[] | Word-level marks for `modified` (FR-011) |
| `state` | HunkState | `unresolved` \| `applied{from}` \| `rejected` \| `manually_edited` |

**Rules**: `category=conflicting` ⇒ `origin=both` and bulk "apply non-conflicting" skips it (Edge Cases, FR-009). A manual center edit inside `result_range` flips `state` to `manually_edited` (FR-012). Adjacent/overlapping hunks keep distinct `id`s and operate independently (FR-010, Edge Cases).

## IntraLineSpan

| Field | Type | Notes |
|-------|------|-------|
| `pane` | Pane | Which pane the span is in |
| `row` | u32 | Aligned result-spine row |
| `start_col` / `end_col` | u32 | Char range of the changed words (FR-011) |

## Alignment

| Field | Type | Notes |
|-------|------|-------|
| `local_rows` | RowMap | result-row → local line or `Filler` |
| `incoming_rows` | RowMap | result-row → incoming line or `Filler` |

**Rules**: Every result row maps to exactly one entry per side (a real line or a filler), keeping the three panes horizontally aligned (FR-005). UI renders `Filler` as blank gutter space; computes no offsets itself.

## Connector (UI-derived, core-supplied endpoints)

| Field | Type | Notes |
|-------|------|-------|
| `hunk_id` | HunkId | Owning region |
| `side` | `local` \| `incoming` | Which gutter |
| `side_range` | LineRange | Endpoint on the side pane |
| `result_range` | LineRange | Endpoint on the result pane |

**Rules**: One connector per side per hunk (`both` ⇒ two connectors to the same `result_range`, Edge Cases). UI anchors endpoints to live line geometry and culls off-screen connectors (R3, SC-004, SC-007).

## Operation (apply/revert)

| Field | Type | Notes |
|-------|------|-------|
| `hunk_id` | HunkId | Target |
| `kind` | `apply{from}` \| `revert` | |
| `before` | ResultSlice | Result content + range before op |
| `after` | ResultSlice | Result content + range after op |

**Rules**: Each op is the inverse of its undo; replaying inverse restores exact prior state (FR-010, SC-005). Scoped to one hunk's `result_range`; never edits other regions. Round-trip apply→revert must restore the original result (constitution Quality Gates).

## ResolutionStatus

| Field | Type | Notes |
|-------|------|-------|
| `total_hunks` | u32 | |
| `remaining_conflicts` | u32 | `conflicting` hunks still `unresolved` |
| `fully_resolved` | bool | `remaining_conflicts == 0` and no unresolved conflicts |

**Rules**: Recomputed after every op/edit (FR-012, FR-013). `fully_resolved=true` signals the file is ready to write out (Edge Cases).

## State transitions (HunkState)

```text
unresolved --apply{from}-->  applied{from}
applied{from} --revert-->    unresolved
unresolved --manual edit-->  manually_edited
applied{from} --manual edit-> manually_edited
manually_edited --revert-->  unresolved   (restores pre-edit slice via Operation log)
any --global undo-->         previous state (inverse Operation)
```
