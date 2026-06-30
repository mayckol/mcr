---

description: "Task list for Multi-File Merge Navigator"
---

# Tasks: Multi-File Merge Navigator

**Input**: Design documents from `/specs/002-multi-file-merge/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/tauri-commands.md, quickstart.md

**Tests**: Included — the constitution's "Development Workflow & Quality Gates" mandates automated
tests for resolution/apply-revert logic plus an end-to-end real `git mergetool` validation.

**Organization**: Grouped by user story (US1, US2 = P1; US3 = P2) for independent implementation and
testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: US1 / US2 / US3 (Setup, Foundational, Polish carry no story label)

## Path Conventions

Rust workspace + `ui/` frontend (see plan.md). Backend: `src-tauri/src/`, `crates/mcr-core/`.
Frontend: `ui/src/`, tests in `ui/tests/`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Scaffold the new modules the feature introduces (project already initialized).

- [x] T001 Create `src-tauri/src/discovery.rs` module stub and declare `mod discovery;` in `src-tauri/src/lib.rs`
- [x] T002 [P] Create UI component file stubs `ui/src/files/list.ts` and `ui/src/confirm/modal.ts`
- [x] T003 [P] Create test file stubs `ui/tests/files_list.test.ts` and `ui/tests/exit_modal.test.ts`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Make the backend discover the conflicted set, model N files, and return a file list with
the list-vs-direct decision. **No user story can begin until this is done.**

- [x] T004 [P] Implement git plumbing in `src-tauri/src/discovery.rs`: `repo_root(from_path)` via `git rev-parse --show-toplevel`, `unmerged_paths(root)` via `git diff --name-only --diff-filter=U --relative`, and `reconstruct_sides(root, path)` reading index stages `git show :1:/:2:/:3:<path>` (research.md R1)
- [x] T005 [P] Add `ConflictKind` enum (`Text`/`Binary`/`DeleteModify`/`BothAdded`) and `MergeFileEntry` struct in `src-tauri/src/manager.rs` (data-model.md)
- [x] T006 Change `Launch` to `{ merge: Vec<MergeFileEntry>, repo_root: Option<String> }` in `src-tauri/src/manager.rs` (depends on T005)
- [x] T007 Rewrite `parse_launch` in `src-tauri/src/lib.rs` to take the one Git-passed 4-tuple, anchor the repo, discover the full unmerged set, build `Vec<MergeFileEntry>`, and fall back to the single file when outside a worktree / discovery fails (research.md R5; depends on T004, T006)
- [x] T008 Add `order: Mutex<Vec<String>>` and `entries: Mutex<HashMap<String, MergeFileEntry>>` to `SessionManager` and an `open_entries(&[MergeFileEntry])` helper in `src-tauri/src/manager.rs` (depends on T005)
- [x] T009 [P] Add wire types `SessionSummary`, `SessionProgress`, and the new `Bootstrap { mode, files, progress, active }` in `src-tauri/src/commands.rs` and mirror them in `ui/src/ipc/types.ts` (data-model.md)
- [x] T010 Rewrite `bootstrap` in `src-tauri/src/commands.rs` to open all discovered entries, return the file list + progress, and set `active` only when exactly one file (FR-001/FR-015; depends on T007, T008, T009)
- [x] T011 Add `list_sessions` and `select_session` (lazy open) commands in `src-tauri/src/commands.rs` and register all new commands in the `invoke_handler` in `src-tauri/src/lib.rs` (depends on T008, T009)
- [x] T012 [P] Add `ipc` wrappers `listSessions`/`selectSession` in `ui/src/ipc/client.ts` (depends on T009)
- [x] T013 [P] [TEST] Unit-test discovery in `src-tauri/src/discovery.rs` (or `src-tauri/tests/`): unmerged-set enumeration and stage reconstruction over a temp repo (quickstart Scenario A/F)

**Checkpoint**: Backend returns a multi-file list and decides list-vs-direct. User stories can begin.

---

## Phase 3: User Story 1 - See and navigate all conflicted files in one session (Priority: P1) 🎯 MVP

**Goal**: One window lists every conflicted file; selecting one opens it in the existing three-pane
editor; the user switches files freely without the session ending, in-progress work preserved.

**Independent Test**: Launch on a 3+ conflicted-file merge → file list with one row per file →
select any row opens it in-pane → switch to another and back without losing work.

### Tests for User Story 1

- [x] T014 [P] [US1] Component test `ui/tests/files_list.test.ts`: rows render one-per-file with path + status, click selects + toggles active-row class, order is stable (FR-012)

### Implementation for User Story 1

- [x] T015 [P] [US1] Add `<aside id="file-list">` beside `#merge-container` (wrap in a horizontal flex row) in `ui/index.html`
- [x] T016 [P] [US1] Add `.file-list` styles reusing the Tokyo Night palette vars in `ui/src/styles.css`
- [x] T017 [US1] Implement the `FileList` component class in `ui/src/files/list.ts` (constructor: root + onSelect callback; `render(summaries, activeId)`; click → select + active-row class), modeled on `ShortcutsPanel`
- [x] T018 [US1] Wire file-list into `ui/src/main.ts`: add module-level `files`/`activeFile`, render the list from `bootstrap`, and on select call `selectSession` → route the model through the existing `apply()` funnel (reuse the singleton `MergeEditor`, do NOT mount a second) (research.md R6; depends on T012, T017)
- [x] T019 [US1] Handle the single-file path in `ui/src/main.ts`: when `bootstrap.active` is set (one file), render the editor directly with no list (FR-015; depends on T018)
- [x] T020 [US1] Preserve in-progress work on switch: ensure switching files reuses each file's live `SessionModel` (no reset) and restores it on return (FR-004; depends on T018)

**Checkpoint**: Multi-file navigation works end-to-end; single-file opens directly. MVP demoable.

---

## Phase 4: User Story 2 - Track per-file status and progress to completion (Priority: P1)

**Goal**: Per-file status updates live, overall progress is shown, resolved files persist
incrementally, and the merge can be finished from one action — with a confirmation modal when files
remain unresolved.

**Independent Test**: Resolve one file → its row flips resolved + progress decrements; resolve all →
finish exits 0; leave one → exit shows the confirmation modal and keeps resolved files.

### Tests for User Story 2

- [x] T021 [P] [US2] Component test `ui/tests/exit_modal.test.ts`: modal lists the remaining unresolved files and confirm/cancel behave (FR-008)
- [x] T022 [P] [US2] Backend test in `src-tauri/tests/`: `finish` reports `all_resolved` only when every entry resolved, and exit-code selection reflects only the Git-passed file (research.md R2; quickstart Scenario C)

### Implementation for User Story 2

- [x] T023 [P] [US2] Implement `save_and_stage(session_id)` in `src-tauri/src/commands.rs`/`manager.rs`: optional `<path>.orig` backup when `mergetool.keepBackup`, write result to worktree path, `git add -- <path>` (research.md R3/R4)
- [x] T024 [US2] Auto-invoke `save_and_stage` when a file reaches `fully_resolved` and update its `MergeFileEntry.resolved`; recompute `SessionProgress` (FR-005/FR-006; depends on T023)
- [x] T025 [US2] Implement `finish` command + `FinishOutcome` in `src-tauri/src/commands.rs`: stage all resolved, gate on every entry resolved, return unresolved list otherwise; register it (FR-007/FR-017; depends on T024)
- [x] T026 [US2] Update `quit` exit-code semantics in `src-tauri/src/commands.rs` to reflect only the Git-passed file (0 = that file resolved, non-zero = aborted) (contracts exit-code table; depends on T025)
- [x] T027 [P] [US2] Render per-file status + the "N of M resolved / K conflicts remaining" progress summary in the file list and toolbar in `ui/src/main.ts` + `ui/src/files/list.ts` (FR-006)
- [x] T028 [US2] Implement the `ExitConfirmModal` component in `ui/src/confirm/modal.ts` reusing the `.mcr-modal-*` CSS (lists unresolved files; confirm/cancel callbacks), modeled on `ShortcutsPanel`
- [x] T029 [US2] Wire the modal into the `#save-exit` / `#abort` handlers in `ui/src/main.ts`: gate on `progress.all_resolved`; on confirm call `finish` then `quit` with the correct code (FR-008; depends on T028, T025)
- [x] T030 [US2] Add `ipc` wrappers `saveAndStage`/`finish` + `FinishOutcome` type in `ui/src/ipc/client.ts` and `ui/src/ipc/types.ts` (depends on T025)

**Checkpoint**: Full to-completion workflow with incremental persistence + safe exit works.

---

## Phase 5: User Story 3 - Resolve whole files from the list and move quickly (Priority: P2)

**Goal**: Accept a whole file's local/incoming side from the list without opening the editor, jump to
the next unresolved file, with reversibility.

**Independent Test**: Accept local on one file and incoming on another from the list → both resolve
with the expected content → "next unresolved" lands on the remaining file → accept is undoable.

### Tests for User Story 3

- [x] T031 [P] [US3] Backend test in `src-tauri/tests/`: `accept_file` resolves to the chosen side, persists, and is reversible via undo (FR-009/FR-010; quickstart Scenario B)

### Implementation for User Story 3

- [x] T032 [US3] Implement `accept_file(session_id, from)` in `src-tauri/src/commands.rs` (apply the chosen side whole-file, mark resolved, `save_and_stage`, record for undo) and register it (FR-009/FR-010; depends on T023)
- [x] T033 [P] [US3] Add per-row "accept local / accept incoming" controls in `ui/src/files/list.ts` calling `acceptFile` and refreshing the list (depends on T032)
- [x] T034 [US3] Implement "next unresolved" action in `ui/src/main.ts` (advance to the next unresolved file in stable `order`) and bind it (FR-011; depends on T018)
- [x] T035 [P] [US3] Add `ipc` wrapper `acceptFile(id, from)` in `ui/src/ipc/client.ts` (depends on T032)
- [x] T036 [P] [US3] Surface special-conflict files (`kind != Text`) in `ui/src/files/list.ts` with a distinct status and accept-only controls (FR-014)

**Checkpoint**: All three stories independently functional.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [x] T037 [P] Centralize `mergetool.keepBackup` reading (`git config --get`) used by `save_and_stage` in `src-tauri/src/discovery.rs`
- [ ] T038 [P] Performance check: file list stays scrollable and select reveals editor < 300 ms at 200 files (SC-003) — add a benchmark or jsdom timing assertion
- [x] T039 [P] Run `bash scripts/vendor-neutral-check.sh` and confirm no vendor reference in new UI strings (constitution VI)
- [ ] T040 End-to-end gate: scripted real `git mergetool` over a multi-file conflict resolving all files, asserting exit 0 + all staged (constitution Development Workflow; quickstart Scenario A)
- [ ] T041 Execute quickstart.md Scenarios B–F manually and record results

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2)**: depends on Setup — **blocks all user stories**.
- **User Stories (Phase 3–5)**: depend on Foundational. US1 and US2 (both P1) can proceed in
  parallel after Phase 2; US3 (P2) depends on `save_and_stage` (T023, in US2).
- **Polish (Phase 6)**: after the desired stories are complete.

### Story Dependencies

- **US1 (P1)**: after Phase 2. No dependency on US2/US3.
- **US2 (P1)**: after Phase 2. Independent of US1 (operates on the same backend list).
- **US3 (P2)**: after Phase 2; reuses `save_and_stage` (T023) from US2 — sequence US2 before US3 or
  pull T023 forward.

### Within Each Story

- Tests before/with implementation; backend command before its UI wiring; component before its
  main.ts wiring.

### Parallel Opportunities

- Setup: T002, T003 in parallel.
- Foundational: T004, T005 in parallel; T009, T012, T013 in parallel once their deps land.
- US1: T014, T015, T016 in parallel.
- US2: T021, T022, T023 in parallel; T027, T030 parallel after their deps.
- US3: T031, T033, T035, T036 parallel after T032.
- Polish: T037, T038, T039 in parallel.

---

## Parallel Example: User Story 1

```bash
# After Phase 2 checkpoint, launch US1's independent tasks together:
Task: "Component test ui/tests/files_list.test.ts (rows, select, stable order)"
Task: "Add <aside id=file-list> beside #merge-container in ui/index.html"
Task: "Add .file-list styles in ui/src/styles.css"
```

---

## Implementation Strategy

### MVP First (US1)

1. Phase 1 Setup → Phase 2 Foundational (backend list + list-vs-direct).
2. Phase 3 US1 (navigate files in one window).
3. **STOP & VALIDATE**: open a 3-file conflict, switch between files, single-file opens direct.

### Incremental Delivery

1. Foundation ready → US1 (navigation MVP).
2. US2 → to-completion workflow (persist + finish + modal).
3. US3 → speed layer (whole-file accept + next-unresolved).
4. Polish → perf, branding lint, e2e mergetool gate.

---

## Notes

- [P] = different files, no incomplete-task dependency.
- The merge engine (`crates/mcr-core`) and the per-session editing commands are reused unchanged.
- Per constitution IV, every apply/accept must remain reversible; verify the round-trip (SC-005).
- Commit after each task or logical group.
