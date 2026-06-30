# Implementation Plan: Multi-File Merge Navigator

**Branch**: `002-multi-file-merge` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/002-multi-file-merge/spec.md`

## Summary

Turn MCR from a one-file-per-process merge editor into a single-window session that lists every
conflicted file in a merge, lets the user navigate between them, resolve each in the existing
three-pane editor (or accept a whole side from the list), tracks per-file status + overall
progress, and finishes the whole merge from one place.

Technical approach: the merge engine (`mcr-core`) and the `SessionManager` store are **already
N-file capable** — `MergeSession` has zero global state and the manager keys sessions by id. The
work is concentrated at three seams: (1) **launch/discovery** — on a `git mergetool` invocation MCR
self-discovers the full unmerged set from the repository (Git only ever hands it one file), (2) a
new **file-list + view-switching + exit-confirmation** UI layer around the singleton three-pane
editor, and (3) **incremental persist + aggregate exit** — each file is written back and staged the
moment it is resolved, and Git's own per-file mergetool loop re-drives MCR to resume any files the
user skipped. No wrapper script is added (constitution III); the existing `mergetool.mcr.cmd`
configuration keeps working unchanged.

## Technical Context

**Language/Version**: Rust (edition 2021) core + Tauri v2 shell; TypeScript 5 (strict) UI on Vite 5

**Primary Dependencies**: `mcr-core` (diff3/alignment/apply-revert, `similar`); Tauri v2
(`@tauri-apps/api`); CodeMirror 6 (`@codemirror/state`, `@codemirror/view`); Git plumbing invoked as
a subprocess (`git` CLI) for unmerged-set discovery and index-stage reconstruction

**Storage**: Filesystem — Git worktree files (per-file `MERGED` output) and the Git index (staging
on resolve). No database. Client persists only keyboard bindings (localStorage)

**Testing**: `cargo test -p mcr-core` (engine round-trip), Rust unit tests in `src-tauri` (discovery
+ aggregation), Vitest + jsdom (UI components), and an end-to-end real `git mergetool` invocation
(constitution Development Workflow gate)

**Target Platform**: Desktop — macOS (Apple Silicon), Linux (x86_64), Windows; single Tauri binary

**Project Type**: Desktop app — Rust workspace (`crates/mcr-core`, `src-tauri`) + `ui/` frontend

**Performance Goals**: File-list responsive and scrollable at 200+ files; selecting a file reveals
its editor < 300 ms (SC-003); per-file apply/revert reflected < 100 ms (feature 001 carries over)

**Constraints**: Must honor Git's mergetool exit-code/`MERGED` contract (constitution III); no
wrapper script beyond documented config; no global state added to the core; lazy per-file editor
open so a large conflict set does not load every editor up front; vendor-neutral UI (constitution VI)

**Scale/Scope**: A merge's conflicted-file set — typically 1–dozens, must stay usable into the
hundreds. Out of scope: read-only branch/commit comparison (deferred follow-up, Principle V)

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Clean-Room Original Implementation | PASS | Reuses MCR's own engine + UI; only the *idea* of a multi-file conflict list is borrowed. No third-party merge-tool code/assets. |
| II. Three-Way Visual Merge Experience | PASS | Per-file resolution is unchanged — the existing three-pane editor. This feature only adds navigation around it. |
| III. Drop-In Git Merge Editor | PASS (design-critical) | Self-discovery keeps the documented `mergetool.mcr.cmd` config working with **no wrapper script**. Exit-code/`MERGED` contract preserved: MCR returns Git's per-file verdict; incremental `git add` lets Git's loop skip already-resolved files. See research.md R2/R3. |
| IV. Reversible, Non-Destructive Operations | PASS | Apply/revert/undo carry over per file. Whole-file accept-from-list is reversible (FR-010). Incremental write+stage is recoverable via Git stages; a per-file `.orig` backup is added because Git's own `keepBackup`/`.orig` machinery does not cover files MCR stages directly (research.md R4). |
| V. First-Class Branch & Commit Comparison | PASS (scoped) | Comparison is explicitly deferred; the file-list is designed so the same component can later back an all-changed-files comparison mode. |
| VI. Vendor-Neutral Branding | PASS | No vendor name/asset in the file list, modal, or any new string. Branding lint (`scripts/vendor-neutral-check.sh`) covers new code. |

**Technology Stack gate**: All discovery/aggregation/exit logic stays in Rust; UI holds no merge
logic (it dispatches intents over `ipc` and renders `SessionModel`). PASS.

**Result**: No violations. Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/002-multi-file-merge/
├── plan.md              # This file
├── research.md          # Phase 0 — discovery/exit-code/persist decisions
├── data-model.md        # Phase 1 — entities & wire shapes
├── quickstart.md        # Phase 1 — runnable validation scenarios
├── contracts/
│   └── tauri-commands.md # Phase 1 — backend command contracts
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
crates/mcr-core/              # UNCHANGED — already per-file isolated, zero global state
└── src/{diff,session,wire,align,...}.rs

src-tauri/src/
├── lib.rs                    # parse_launch: 1 tuple → discover N; register new commands
├── manager.rs                # Launch.merge: Option<MergeFiles> → Vec; add order + per-file labels;
│                             #   add finish/aggregate-exit + write-and-stage helper
├── commands.rs               # bootstrap → list-many; new: list_sessions, select_session,
│                             #   accept_file (whole-file), save_and_stage, finish, confirm-exit gate
├── discovery.rs  (NEW)       # git plumbing: repo root, unmerged set, index-stage reconstruction
└── main.rs

ui/
├── index.html                # add <aside id="file-list"> beside #merge-container (flex row)
├── src/
│   ├── main.ts               # add files/activeFile state; wire sidebar + modal into apply() funnel
│   ├── files/list.ts (NEW)   # file-list sidebar component (ShortcutsPanel-style class)
│   ├── confirm/modal.ts (NEW)# exit-confirmation modal (reuse .mcr-modal-* CSS)
│   ├── ipc/{client,types}.ts # add wrappers + wire types for new commands + filename/summary fields
│   └── styles.css            # .file-list rules reusing Tokyo Night palette vars
└── tests/                    # files_list.test.ts, exit_modal.test.ts (Vitest + jsdom)
```

**Structure Decision**: Keep the existing Rust-workspace-plus-`ui/` layout. The merge engine and the
multi-session store need no structural change; new backend code is one `discovery.rs` module plus
additions to `lib.rs`/`manager.rs`/`commands.rs`. New UI code follows the established
component-class pattern (`ShortcutsPanel`) and the single render funnel (`apply()`), reusing the one
`MergeEditor` instance for every file rather than mounting one editor per file (the decoration
registry is keyed globally per pane name — a second editor would clobber it).

## Phase 0: Outline & Research

See [research.md](./research.md). Resolves: launch discovery vs. wrapper/manifest (R1), the
exit-code strategy that reconciles a single window with Git's per-file loop (R2), incremental
persist + resume mechanics (R3), the `.orig`/backup gap and reversibility (R4), the
single-vs-multi/outside-worktree fallback (R5), and the UI singleton-reuse + lazy-open approach (R6).

## Phase 1: Design & Contracts

- [data-model.md](./data-model.md): multi-file `Launch`, `MergeFileEntry`, `SessionSummary`,
  `SessionProgress`, and the wire additions (`filename`/`path_label`, list payload).
- [contracts/tauri-commands.md](./contracts/tauri-commands.md): the new/changed Tauri command
  surface (`bootstrap`→list, `list_sessions`, `select_session`, `accept_file`, `save_and_stage`,
  `finish`) and the exit-confirmation gate, plus exit-code semantics.
- [quickstart.md](./quickstart.md): end-to-end validation via a real multi-file `git mergetool` run,
  plus UI component scenarios.

## Complexity Tracking

No constitution violations — section intentionally empty.
