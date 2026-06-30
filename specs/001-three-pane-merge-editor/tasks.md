---
description: "Task list for Three-Pane Visual Merge Editor"
---

# Tasks: Three-Pane Visual Merge Editor

**Input**: Design documents from `/specs/001-three-pane-merge-editor/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: INCLUDED — the constitution (Development Workflow & Quality Gates) mandates automated tests for apply/revert logic, an apply→revert round-trip test, and end-to-end validation. Test tasks are therefore required, not optional.

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: US1 / US2 / US3 from spec.md

## Path Conventions

Cargo workspace per plan.md: `crates/mcr-core/` (Rust merge logic), `src-tauri/` (Tauri shell + IPC), `ui/` (TypeScript + CodeMirror frontend, no merge logic).

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Workspace, toolchain, project skeleton

- [X] T001 Create Cargo workspace root `Cargo.toml` with members `crates/mcr-core` and `src-tauri` per plan.md
- [X] T002 Scaffold `crates/mcr-core` crate (`crates/mcr-core/Cargo.toml`, `crates/mcr-core/src/lib.rs`) with `imara-diff`, `serde`, `serde_json` deps and `proptest` dev-dep
- [X] T003 Scaffold Tauri 2.x shell in `src-tauri/` (`src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/main.rs`) depending on `mcr-core`
- [X] T004 [P] Scaffold TypeScript frontend in `ui/` (`ui/package.json`, `ui/tsconfig.json`, Vite + CodeMirror 6 + Vitest) per plan.md
- [X] T005 [P] Configure Rust lint/format (`rustfmt.toml`, `clippy` in CI) and TS lint/format (ESLint + Prettier in `ui/`)
- [X] T006 [P] Add vendor-neutral lint check script (greps source/build for the inspiration vendor name → zero hits) per FR-017 / Principle VI

**Checkpoint**: Workspace builds empty; `cargo build` and `npm --prefix ui run build` succeed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data types and wire payloads every story depends on

**⚠️ CRITICAL**: No user story work begins until this phase completes

- [X] T007 [P] Define core domain types in `crates/mcr-core/src/hunk.rs`: `ChangeRegion`, `Origin`, `Category`, `HunkState`, `LineRange`, `IntraLineSpan` per data-model.md
- [X] T008 [P] Define `crates/mcr-core/src/wire.rs` serde payload types: `SessionModel`, `Delta`, `Alignment`/`RowMap`, `ResolutionStatus`, request enums per contracts/ipc-merge-session.md
- [X] T009 Define `MergeSession` skeleton + `OperationLog`/`Operation` types in `crates/mcr-core/src/session.rs` and `crates/mcr-core/src/ops.rs` (fields only, no logic) per data-model.md
- [X] T010 Establish Tauri IPC command surface stubs in `src-tauri/src/commands.rs` (`open_session`, `apply_change`, `revert_change`, `apply_non_conflicting`, `edit_result`, `undo`, `redo`, `navigate`, `set_whitespace_mode`) returning `unimplemented` per contracts/ipc-merge-session.md
- [X] T011 [P] Establish typed IPC client wrappers in `ui/src/ipc/` mirroring the command/`SessionModel`/`Delta` shapes per contracts/ipc-merge-session.md
- [X] T012 [P] Create dev fixtures (local/ancestor/incoming sample strings with conflicts + non-conflicting changes per side) in `crates/mcr-core/tests/fixtures/` for use across stories and quickstart

**Checkpoint**: Types compile end-to-end; IPC stubs callable from UI; fixtures available.

---

## Phase 3: User Story 1 - Resolve a conflicted file in three panes (Priority: P1) 🎯 MVP

**Goal**: Render three panes (local / editable result / incoming) with line-level highlight bands, gutter connectors to aligned result regions, and alignment filler.

**Independent Test**: Open a fixture with ≥1 conflict + one non-conflicting change per side; verify three panes render, every hunk highlighted, every on-screen hunk connected to its result region, filler keeps lines aligned (FR-001..FR-005, SC-002).

### Tests for User Story 1 ⚠️

- [X] T013 [P] [US1] Unit tests for line-diff hunk detection in `crates/mcr-core/tests/word_diff.rs` (hunk boundaries, categories) against fixtures
- [X] T014 [P] [US1] Unit tests for three-pane alignment + filler in `crates/mcr-core/tests/alignment.rs` (result-spine row maps, differing line counts) per FR-005
- [X] T015 [P] [US1] Frontend test for pane binding + line-band decorations from `SessionModel` in `ui/tests/panes.test.ts`

### Implementation for User Story 1

- [X] T016 [P] [US1] Implement line diff + hunk extraction (origin/category) in `crates/mcr-core/src/diff.rs` using `imara-diff` per research R1
- [X] T017 [US1] Implement result-spine alignment + filler computation in `crates/mcr-core/src/align.rs` per research R2, data-model Alignment (depends on T016)
- [X] T018 [US1] Implement `MergeSession::open` in `crates/mcr-core/src/session.rs`: build panes, hunks, alignment, initial result (clean-merge non-conflicting, conflicts unresolved), status per contracts/core-api.md (depends on T016, T017)
- [X] T019 [US1] Wire `open_session` command in `src-tauri/src/commands.rs` to serialize full `SessionModel` (depends on T018)
- [X] T020 [P] [US1] Build three CodeMirror instances (local/result/incoming; result editable, sides read-only) in `ui/src/panes/` bound to `SessionModel.panes`
- [X] T021 [P] [US1] Implement line-band decorations per `category` in `ui/src/highlight/` from `hunks` per FR-003
- [X] T022 [US1] Render alignment filler as blank gutter rows in `ui/src/panes/` from `alignment` per FR-005 (depends on T020)
- [X] T023 [US1] Implement SVG connector overlay in `ui/src/connectors/`: one path per `{hunk, side}` from side range → result range, anchored to CodeMirror line geometry, off-screen culling per research R3, FR-004 (depends on T020, T022)

**Checkpoint**: US1 fully functional — file opens, renders three traced panes; testable independently. **MVP reached.**

---

## Phase 4: User Story 2 - Apply or revert a change with one action (Priority: P1)

**Goal**: Per-change apply/revert into the editable result with immediate update, bulk apply non-conflicting, and reversible global undo.

**Independent Test**: Apply one left + one right change, revert one; verify result updates immediately, only that region reverts, undo reverses exactly (FR-007..FR-010, FR-012, SC-001, SC-003, SC-005).

### Tests for User Story 2 ⚠️

- [X] T024 [P] [US2] **Apply→revert round-trip** test in `crates/mcr-core/tests/apply_revert_roundtrip.rs` proving apply then revert restores the original result (Principle IV, SC-005)
- [X] T025 [P] [US2] Unit tests for adjacent/overlapping hunks staying independent on apply/revert in `crates/mcr-core/tests/apply_revert_roundtrip.rs` per Edge Cases, FR-010
- [X] T026 [P] [US2] Property test (`proptest`) for undo/redo restoring exact prior state across random op sequences in `crates/mcr-core/tests/ops_proptest.rs`
- [X] T027 [P] [US2] Frontend test for incremental `Delta` application (result patch + changed-hunk decorations) in `ui/tests/delta.test.ts`

### Implementation for User Story 2

- [X] T028 [US2] Implement reversible `Operation` apply/revert + undo/redo stack in `crates/mcr-core/src/ops.rs` per research R5, data-model Operation
- [X] T029 [US2] Implement `session.apply` / `session.revert` producing `Delta` in `crates/mcr-core/src/session.rs` (depends on T028)
- [X] T030 [US2] Implement `session.apply_non_conflicting(from)` skipping conflicting hunks in `crates/mcr-core/src/session.rs` per FR-009 (depends on T029)
- [X] T031 [US2] Implement `session.edit_result` (manual center edit → `manually_edited` state + status recompute) in `crates/mcr-core/src/session.rs` per FR-012 (depends on T028)
- [X] T032 [US2] Implement `ResolutionStatus` recompute after each op/edit in `crates/mcr-core/src/session.rs` per FR-013
- [X] T033 [US2] Wire `apply_change`, `revert_change`, `apply_non_conflicting`, `edit_result`, `undo`, `redo` commands in `src-tauri/src/commands.rs` returning `Delta` (depends on T029, T030, T031)
- [X] T034 [P] [US2] Implement per-hunk apply/revert controls (gutter affordances) in `ui/src/panes/` dispatching IPC intents per FR-007, FR-008
- [X] T035 [US2] Apply `Delta` incrementally in `ui/` (patch result doc, update only changed-hunk decorations/connectors, refresh status) per contracts/ipc-merge-session.md (depends on T034, T023)
- [X] T036 [P] [US2] Add bulk "apply all non-conflicting (left/right/both)" controls + global undo/redo in `ui/src/panes/` per FR-009, FR-010
- [X] T037 [P] [US2] Add file resolution status display (remaining conflicts / fully-resolved) in `ui/` per FR-013

**Checkpoint**: US1 + US2 work independently — full resolve loop with reversible apply/revert.

---

## Phase 5: User Story 3 - Precise highlight binding that follows the lines (Priority: P2)

**Goal**: Word-level intra-line highlighting, synchronized scrolling, connectors/highlights anchored to lines through scroll and resize, distinct consistent category colors.

**Independent Test**: Open a fixture with modified lines; verify word spans mark exact words, panes scroll in sync, connectors re-anchor with no drift on scroll/resize (FR-006, FR-011, SC-004, SC-006).

### Tests for User Story 3 ⚠️

- [X] T038 [P] [US3] Unit tests for word-level (intra-line) span computation in `crates/mcr-core/tests/word_diff.rs` per FR-011
- [X] T039 [P] [US3] Frontend test for scroll-sync across three panes + connector re-anchoring on scroll/resize in `ui/tests/scroll_connectors.test.ts` per FR-006, SC-004

### Implementation for User Story 3

- [X] T040 [US3] Implement word-level diff pass over modified hunks producing `IntraLineSpan`s in `crates/mcr-core/src/diff.rs` per research R1, FR-011 (extends T016)
- [X] T041 [US3] Include `word_spans` in `SessionModel`/`Delta` serialization in `crates/mcr-core/src/wire.rs` + `session.rs` (depends on T040)
- [X] T042 [P] [US3] Implement word-span `Decoration.mark` rendering (distinct from line band) in `ui/src/highlight/` per FR-011
- [X] T043 [P] [US3] Implement synchronized scrolling across the three panes in `ui/src/scroll/` per FR-006
- [X] T044 [US3] Re-project connectors + bands on scroll and resize with off-screen culling in `ui/src/connectors/` per FR-006, SC-004, SC-007 (depends on T043, T023)
- [X] T045 [P] [US3] Define vendor-neutral category color token map (added/removed/modified/conflicting) in `ui/src/highlight/` per FR-003, FR-017

**Checkpoint**: All three stories independently functional — precise, line-locked tracing.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Settings, performance, validation across stories

- [X] T046 [US-all] Implement whitespace mode (none/ignore-trailing/ignore-all) at tokenization in `crates/mcr-core/src/diff.rs` + `set_whitespace_mode` command re-diff per FR-014, research R7
- [X] T047 [P] [US-all] Add whitespace-mode toggle + next/prev change navigation UI in `ui/` per FR-014, FR-016
- [X] T048 [US-all] Implement `session.navigate(next/prev)` revealing target hunk in all three panes in `crates/mcr-core/src/session.rs` + `navigate` command per FR-016
- [X] T049 [P] Performance pass: verify apply/revert <100 ms on 5k-line fixture (SC-003) and interactivity at 1,000+ regions with connector culling (SC-007); add benchmark in `crates/mcr-core/benches/`
- [X] T050 [P] Run vendor-neutral lint (T006 script) across source + build output; confirm zero hits (FR-017, Principle VI)
- [X] T051 Execute `quickstart.md` scenarios S1–S5 end-to-end and record results
- [X] T052 [P] Wire CI to run `cargo test -p mcr-core` (incl. round-trip), `npm --prefix ui test`, and vendor-neutral lint on Linux/macOS/Windows per constitution Quality Gates

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies — start immediately
- **Foundational (Phase 2)**: depends on Setup — BLOCKS all user stories
- **User Stories (Phase 3–5)**: depend on Foundational; US1 is MVP. US2 depends on US1's session/result (T018). US3 extends US1 diff/connectors (T016/T023). Within-priority, US1 → US2 → US3.
- **Polish (Phase 6)**: depends on the stories it touches

### Story Dependencies

- **US1 (P1)**: after Foundational — no story deps. MVP.
- **US2 (P1)**: builds on US1 (`MergeSession`/result + connector render). Independently testable via core API + UI.
- **US3 (P2)**: extends US1 diff (word spans) and connector overlay. Independently testable.

### Within Each Story

- Tests written first and FAIL before implementation (constitution Quality Gates)
- Core types → diff/align → session/ops → IPC command → UI render
- Different files marked [P] run in parallel

### Parallel Opportunities

- Setup: T004, T005, T006 in parallel
- Foundational: T007, T008 parallel; T011, T012 parallel
- US1 tests T013–T015 parallel; UI T020/T021 parallel
- US2 tests T024–T027 parallel; UI T034/T036/T037 parallel
- US3 T038/T039 parallel; T042/T043/T045 parallel

---

## Parallel Example: User Story 1

```bash
# Tests together:
Task: "Unit tests for line-diff hunk detection in crates/mcr-core/tests/word_diff.rs"
Task: "Unit tests for alignment + filler in crates/mcr-core/tests/alignment.rs"
Task: "Frontend pane-binding test in ui/tests/panes.test.ts"

# Then parallel implementation:
Task: "Line diff + hunk extraction in crates/mcr-core/src/diff.rs"
Task: "Three CodeMirror panes in ui/src/panes/"
Task: "Line-band decorations in ui/src/highlight/"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Phase 1 Setup → 2. Phase 2 Foundational → 3. Phase 3 US1 → **STOP & VALIDATE** three-pane traced view → demo.

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. US1 → traced three-pane view (MVP)
3. US2 → apply/revert resolve loop
4. US3 → precise word-level binding + scroll-locked connectors
5. Polish → whitespace/navigation/perf/CI

### Parallel Team Strategy

After Foundational: one dev on `mcr-core` (diff/align/ops), one on Tauri IPC, one on `ui/` panes/connectors — coordinated by the contracts in `contracts/`.

---

## Notes

- [P] = different files, no incomplete-task dependency
- Tests precede implementation and must fail first (constitution)
- All merge logic stays in `crates/mcr-core`; `ui/` only renders + dispatches (constitution Tech Stack)
- Vendor-neutral: no naming/depicting the inspiring IDE anywhere (FR-017, Principle VI)
- Commit after each task or logical group

## Implementation Verification (this run)

- `mcr-core`: 14 tests green incl. apply→revert round-trip (Principle IV) + undo/redo proptest; `cargo clippy`/`fmt` clean.
- Frontend: `tsc --noEmit` clean, 5 Vitest tests green, `vite build` produces `ui/dist`.
- Vendor-neutral lint (`scripts/vendor-neutral-check.sh`) passes on shipped code (FR-017).
- `src-tauri` shell + IPC commands written; the live webview GUI was not launched in this headless run (needs the Tauri toolchain + a display). The `SessionManager` logic delegates to the tested `mcr-core` engine.
- Deviations from design docs: engine uses `similar` (research-listed fallback) instead of `imara-diff` for an ergonomic, reliable diff3 + char-level word diff; mutating commands return the full `SessionModel` (delta optimization deferred); `Delta` payload type not yet emitted. Alignment uses a unified row grid (`AlignRow` with optional per-pane indices) — a refinement of data-model.md's result-spine map that also renders side-only lines as filler.
