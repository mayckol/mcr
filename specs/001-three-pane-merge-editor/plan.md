# Implementation Plan: Three-Pane Visual Merge Editor

**Branch**: `001-three-pane-merge-editor` | **Date**: 2026-06-30 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/001-three-pane-merge-editor/spec.md`

## Summary

Build the three-pane visual merge editor: local / editable-result / incoming panes with line-level highlight bands, gutter connectors that bind each change to its aligned result region, word-level intra-line highlighting, scroll-synced panes, and per-change apply/revert with reversible undo. All diff/merge/alignment/apply/revert logic lives in a Rust core crate (`mcr-core`); the UI is a TypeScript + CodeMirror frontend in a Tauri webview shell that only renders state the core computes and dispatches user intents back to it. This feature covers the editor experience; the `git mergetool` entrypoint and file I/O are separate features that consume the same core.

## Technical Context

**Language/Version**: Rust 1.79+ (core + Tauri shell); TypeScript 5.x (frontend)

**Primary Dependencies**: `imara-diff` (line + word diff), Tauri 2.x (webview shell + IPC), CodeMirror 6 (three editor instances), `serde`/`serde_json` (core↔UI payloads). Connectors drawn with SVG overlay in the frontend.

**Storage**: N/A — in-memory merge session; no persistence. The merged result is held in memory until a separate write-out feature persists it.

**Testing**: `cargo test` + `proptest` for core (diff, alignment, apply/revert round-trip); Vitest + CodeMirror test harness for frontend rendering/scroll-sync; Tauri integration test for IPC contract.

**Target Platform**: Desktop (Linux, macOS, Windows) via single Tauri binary.

**Project Type**: Desktop app — Rust workspace (core crate + Tauri shell) + TypeScript frontend.

**Performance Goals**: Apply/revert reflected on screen <100 ms for files ≤5,000 lines (SC-003); interactive scroll/navigate/apply with ≥1,000 change regions, no stutter (SC-007); connector re-anchor on scroll/resize with no perceptible drift (SC-004); cold-start fast enough for per-invocation merge use (constitution Tech Stack).

**Constraints**: Frontend holds NO merge logic (constitution); all resolution state derived from core. Non-destructive: working file untouched until explicit write-out (FR-015). Vendor-neutral: no naming/depicting the inspiring IDE anywhere (FR-017, Principle VI). Connectors render only for on-screen regions (Edge Cases / SC-007).

**Scale/Scope**: Single-file merge sessions; files up to ~5k lines as the SC target, with graceful behavior on larger files and 1,000+ change regions.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle / Section | Gate | Status |
|---------------------|------|--------|
| I. Clean-Room Original Implementation | Every dependency is a general-purpose library (diff engine, webview, code editor) — none is a repackaged merge editor; only the *interaction idea* is reproduced. | PASS |
| II. Three-Way Visual Merge | Plan delivers three panes (local / result / incoming) with per-hunk accept/reject/edit and live result (FR-001, FR-007, FR-008, FR-012). | PASS |
| III. Drop-In Git Merge Editor | Merge logic isolated in `mcr-core` so the separate mergetool entrypoint can drive the same engine; this feature doesn't touch exit-code/`MERGED` contract. | PASS |
| IV. Reversible, Non-Destructive | Apply/revert symmetric and undoable (FR-010); inspection read-only, no working-file mutation until write-out (FR-015); round-trip apply→revert test required. | PASS |
| V. First-Class Compare | Architecture (core computes hunks between two refs) does not preclude reusing the same diff/alignment for branch/commit compare later. | PASS |
| VI. Vendor-Neutral Branding | No user-facing text/asset/identifier names or implies the inspiring IDE (FR-017); review gate enforces. | PASS |
| Technology Stack | Rust core; Tauri webview (not Electron); TS + CodeMirror frontend with zero merge logic; single cross-platform workspace. | PASS |

No violations. Complexity Tracking not required.

## Project Structure

### Documentation (this feature)

```text
specs/001-three-pane-merge-editor/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (core↔UI IPC + core API)
│   ├── core-api.md
│   └── ipc-merge-session.md
└── checklists/
    └── requirements.md  # from /speckit-specify
```

### Source Code (repository root)

```text
crates/
└── mcr-core/                  # Rust: all merge logic
    ├── src/
    │   ├── lib.rs
    │   ├── diff.rs            # line + word (intra-line) diff via imara-diff
    │   ├── align.rs          # three-pane line alignment + filler computation
    │   ├── session.rs        # MergeSession: versions, hunks, result, status
    │   ├── hunk.rs           # ChangeRegion model + categories
    │   ├── ops.rs            # apply/revert as reversible actions + undo stack
    │   └── wire.rs           # serde payload types shared with UI
    └── tests/
        ├── alignment.rs
        ├── apply_revert_roundtrip.rs   # IV: apply→revert restores original
        └── word_diff.rs

src-tauri/                     # Tauri shell + IPC commands
├── src/
│   ├── main.rs
│   └── commands.rs           # open_session, apply_change, revert_change, undo, navigate
├── tauri.conf.json
└── Cargo.toml

ui/                            # TypeScript frontend (no merge logic)
├── src/
│   ├── main.ts
│   ├── panes/                # three CodeMirror instances (local/result/incoming)
│   ├── highlight/            # line-band + word-span decorations from core payload
│   ├── connectors/           # SVG overlay binding side hunks → result, scroll-anchored
│   ├── scroll/               # synchronized scrolling across panes
│   └── ipc/                  # typed wrappers over Tauri commands
└── tests/

Cargo.toml                    # workspace: crates/mcr-core + src-tauri
```

**Structure Decision**: Cargo workspace with `crates/mcr-core` holding 100% of merge/diff/alignment/apply-revert logic, `src-tauri` exposing it over IPC, and `ui/` rendering panes/highlights/connectors. This enforces the constitution's "no merge logic in the frontend" rule and lets the future mergetool entrypoint and compare feature reuse `mcr-core` directly.

## Complexity Tracking

No constitution violations — section intentionally empty.
