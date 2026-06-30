# Phase 1 Contracts: Tauri Command Surface

MCR's UI⇄core boundary is the set of `#[tauri::command]` functions (registered in `lib.rs`
`invoke_handler`) the frontend calls via the typed `ipc` wrappers (`ui/src/ipc/client.ts`). This
contract lists the **new and changed** commands for the multi-file session. The 8 existing
per-session editing commands (`apply_change`, `revert_change`, `apply_non_conflicting`,
`edit_result`, `undo`, `redo`, `navigate`, `set_whitespace_mode`) are **unchanged** — they already
take `session_id` and are multi-file-ready.

Error convention (unchanged): commands return `Result<T, String>`; the `String` is a user-surfaceable
message.

---

## `bootstrap` (CHANGED)

First UI call. Discovers the conflicted set and opens the session.

- **In**: managed `State<Launch>`, `State<SessionManager>`
- **Out**: `Bootstrap { mode, files, progress, active }`
  - `mode`: `"merge"` | `"demo"`
  - `files`: `Vec<SessionSummary>` (empty in demo; 1 in single-file; N in multi-file)
  - `progress`: `SessionProgress`
  - `active`: `Option<SessionModel>` — `Some(model)` when exactly one file (open directly, FR-015);
    `None` when N>1 (list shown first, FR-001)
- **Behavior**: anchors repo root, runs discovery (research.md R1/R5), builds `MergeFileEntry` per
  conflicted path, opens sessions into `SessionManager`. Demo/standalone when `Launch.merge` empty.

## `list_sessions` (NEW)

Return the current file list for the sidebar (re-fetched after state changes).

- **In**: `State<SessionManager>`
- **Out**: `{ files: Vec<SessionSummary>, progress: SessionProgress }`
- **Behavior**: projects `MergeFileEntry` + per-session `ResolutionStatus`; preserves `order`
  (FR-012). Pure read.

## `select_session` (NEW)

Load a specific file's full model when the user picks it from the list (lazy open, SC-003).

- **In**: `session_id: String`, `State<SessionManager>`
- **Out**: `SessionModel`
- **Behavior**: opens the file's `MergeSession` on first selection if not yet opened; returns its
  model for the three-pane editor. Idempotent.

## `accept_file` (NEW — whole-file resolution from the list)

Resolve a whole file to one side without opening the editor (FR-009, US3).

- **In**: `session_id: String`, `from: String` (`"local"` | `"incoming"`), `State<SessionManager>`
- **Out**: `SessionSummary` (updated row) — or `Result<SessionSummary, String>`
- **Behavior**: applies the chosen side to the whole file (reuses `apply_non_conflicting` semantics
  / direct side selection), marks `resolved`, then performs `save_and_stage`. Reversible (FR-010):
  recorded so a subsequent `undo`/revert returns the file to unresolved.

## `save_and_stage` (NEW — incremental persist primitive)

Write a resolved file's result and stage it (research.md R3).

- **In**: `session_id: String`, `State<SessionManager>`, `State<Launch>` (repo root)
- **Out**: `Result<(), String>`
- **Behavior**: (1) optional `<path>.orig` backup when `mergetool.keepBackup` (research.md R4);
  (2) write the session's result pane to `worktree_path` (existing `save_merged` write);
  (3) `git -C <root> add -- <path>`. Called automatically when a file reaches `fully_resolved` or via
  `accept_file`. No-op staging when `repo_root` is `None` (standalone fallback).

## `finish` (NEW — aggregate completion / exit gate)

Complete the whole merge from one action (FR-007, FR-008, FR-017).

- **In**: `State<SessionManager>`, `State<Launch>`
- **Out**: `Result<FinishOutcome, String>` where
  `FinishOutcome { all_resolved: bool, unresolved: Vec<String> }`
- **Behavior**:
  - Ensures every resolved file is written+staged (idempotent `save_and_stage`).
  - If every `MergeFileEntry.resolved` → returns `all_resolved: true`; the UI then calls `quit(0)`
    for the Git-passed file. Git's loop finds the rest staged → skips → mergetool ends.
  - If any remain → returns `all_resolved: false` + the unresolved `path_label`s; the UI shows the
    confirmation modal. **Does not exit by itself.**

## `quit` (CHANGED semantics, same signature)

- **In**: `code: i32`
- **Behavior**: `std::process::exit(code)`. Exit code reflects **only the file Git handed this
  invocation** (research.md R2):
  - `0` — the Git-passed file is resolved (and written). Git marks it resolved, advances its loop;
    other staged files are skipped; unresolved files are re-driven by Git's loop / a later
    `git mergetool`.
  - non-zero — the Git-passed file is not resolved (true abort of this file). Already-staged files
    remain staged (persisted). Always preceded by the confirmation modal when unresolved files exist.

---

## Frontend `ipc` wrappers (`ui/src/ipc/client.ts`) + wire types (`types.ts`)

Add typed wrappers `listSessions()`, `selectSession(id)`, `acceptFile(id, from)`,
`saveAndStage(id)`, `finish()`, and extend `Bootstrap` + add `SessionSummary` / `SessionProgress` /
`FinishOutcome` to `types.ts` mirroring the Rust shapes above. Existing `saveMerged` / `quit` /
per-hunk wrappers stay.

## Exit-code contract summary (constitution III)

| Situation (this invocation) | MCR exit | Git result |
|-----------------------------|----------|------------|
| Git-passed file resolved, all others resolved+staged | `0` | All resolved; loop ends |
| Git-passed file resolved, some others left | `0` | This file resolved; loop re-invokes MCR for the next unmerged → resume |
| Git-passed file left unresolved (user aborts) | non-zero | This file aborted; staged files persist for a later run |

`mergetool.mcr.trustExitCode true` is required (asserted in quickstart) so Git honors these codes.
