# Phase 1 Data Model: Multi-File Merge Navigator

Entities and wire shapes. The per-file merge data (`SessionModel`, `Panes`, `ChangeRegion`,
`AlignRow`, `ResolutionStatus`) is **unchanged** from feature 001 and the existing `mcr-core`
`wire.rs`. This feature adds the *collection* layer around it.

## Backend types (`src-tauri/src`)

### `MergeFiles` (existing — unchanged)
The four paths Git's mergetool contract passes for one file.
- `local: String`, `base: String`, `remote: String`, `merged: String`

### `MergeFileEntry` (NEW)
One conflicted file the session manages.
- `id: String` — the `session_id` ("session-N") used by every per-file command.
- `path_label: String` — repo-relative display path (from `git diff --name-only --relative`).
- `worktree_path: String` — absolute write-back target (`$MERGED` for the Git-passed file, else
  `<root>/<path>`).
- `files: MergeFiles` — the four sources (reconstructed from index stages for non-Git-passed files).
- `kind: ConflictKind` — `Text` | `Binary` | `DeleteModify` | `BothAdded`.
- `resolved: bool` — derived from the session's `status.fully_resolved`, or set by a whole-file
  accept; drives the list status and the finish gate.

### `ConflictKind` (NEW enum)
Distinguishes files resolvable in the text three-pane editor from those needing a per-file choice
(FR-002, FR-014). `Binary` / `DeleteModify` / `BothAdded` render a special status + accept-only UI.

### `Launch` (CHANGED)
- Before: `Launch { merge: Option<MergeFiles> }`
- After: `Launch { merge: Vec<MergeFileEntry>, repo_root: Option<String> }`
  - empty `merge` → standalone/demo mode (preserves today's fallback).
  - `repo_root: None` → launched outside a worktree → single-file fallback (research.md R5).

### `SessionManager` (mostly unchanged)
Already a multi-session store keyed by `session_id`. Additions:
- `order: Mutex<Vec<String>>` — stable list order (discovery order, FR-012).
- Per-file metadata map `entries: Mutex<HashMap<String, MergeFileEntry>>` (path_label, kind,
  worktree_path, resolved) so list/finish do not recompute.
- No change to the existing `sessions` / `merged_paths` maps or the 8 per-session editing commands.

## Wire payloads (`SessionModel` unchanged; new summary types)

### `SessionSummary` (NEW — list row)
Lightweight per-file row for the sidebar; avoids shipping full models for unopened files (lazy open,
SC-003).
- `session_id: String`
- `path_label: String`
- `kind: ConflictKind`
- `resolved: bool`
- `remaining_conflicts: usize` — from the file's `ResolutionStatus` (0 if not yet opened ⇒ shown as
  "unresolved" until opened, unless a whole-file accept already resolved it).

### `SessionProgress` (NEW — derived summary, FR-006)
- `total: usize` — total conflicted files.
- `resolved_count: usize`
- `remaining_conflicts: usize` — files not yet resolved.
- `all_resolved: bool` — gates the single "finish" action (FR-007) and the exit-confirmation modal
  (FR-008/FR-017).

### `Bootstrap` (CHANGED)
- Before: `{ mode: String, model: Option<SessionModel> }`
- After: `{ mode: String, files: Vec<SessionSummary>, progress: SessionProgress, active: Option<SessionModel> }`
  - `mode`: "merge" | "demo".
  - `files`: empty in demo; one entry in single-file mode; N in multi-file mode.
  - `active`: the model to render immediately — the single file in single-file mode, else `None`
    (the list is shown first, FR-001).

## Relationships

- `Launch.merge: Vec<MergeFileEntry>` → on bootstrap, each entry opens (eagerly or lazily) into a
  `MergeSession` stored in `SessionManager`, keyed by `MergeFileEntry.id == session_id`.
- `SessionSummary` is the projection of `MergeFileEntry` + the session's `ResolutionStatus` for the
  UI list.
- `SessionProgress` is the fold over all `MergeFileEntry.resolved` / per-session
  `remaining_conflicts`.

## State transitions (per file)

```
Unresolved ──open in editor, reach fully_resolved──▶ Resolved ──(write+git add)──▶ Persisted
Unresolved ──whole-file accept local/incoming (from list)──▶ Resolved ──(write+git add)──▶ Persisted
Resolved   ──user reopens & reintroduces a conflict (manual edit)──▶ Unresolved
Persisted  ──undo / revert within session──▶ Unresolved   (re-stage on next resolve)
```

- Session-level: `all_resolved` flips true only when every `MergeFileEntry.resolved`. The
  exit-confirmation modal triggers when the user finishes/exits while `all_resolved == false`,
  listing the unresolved `path_label`s.

## Validation rules

- A file's `resolved` MUST reflect live status, including reverting to `false` after a conflicting
  manual edit (FR-005).
- `order` MUST NOT reorder as statuses change (FR-012).
- Special-conflict files (`kind != Text`) MUST surface a distinct status and an accept-only path
  (FR-014); they never open the text three-pane editor.
- `finish` MUST refuse to report overall success unless every `MergeFileEntry.resolved` and every
  write+stage succeeded (FR-017).
