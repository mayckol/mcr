# Phase 0 Research: Three-Pane Visual Merge Editor

Stack is constrained by the constitution (Rust core, Tauri webview, TS + CodeMirror frontend, no merge logic in the frontend). Research below resolves the open choices *within* those constraints.

## R1 — Diff engine for line + word-level diff

**Decision**: Use `imara-diff` for both line-level hunk detection and word-level (intra-line) spans.

**Rationale**: Constitution names `imara-diff`/`similar` as acceptable general-purpose engines (Principle I — no repackaged merge tool). `imara-diff` is fast (Histogram/Myers), token-generic (tokenize by line for hunks, then re-diff each modified hunk by word/grapheme for FR-011), and has low overhead suited to SC-007's 1,000+ regions and SC-003's <100 ms target. Word spans come from a second pass over the paired changed lines.

**Alternatives considered**: `similar` — ergonomic and supports word diff out of the box, but slower on large inputs; kept as fallback. Hand-rolled Myers — rejected, reinventing a solved general-purpose problem with no benefit (and Principle I favors a vetted general library).

## R2 — Three-pane line alignment & filler

**Decision**: Compute alignment in `mcr-core` (`align.rs`) by diffing local↔result and incoming↔result against the merged result as the spine, producing per-pane line→row maps with explicit filler rows where a side has no corresponding line.

**Rationale**: FR-005 requires corresponding lines stay horizontally aligned across panes even with differing line counts; the result pane is the editable spine (FR-002), so anchoring both sides to result rows keeps a single coordinate system for highlights and connectors. Filler rows are data the UI renders as blank gutter space — UI does no alignment math (constitution).

**Alternatives considered**: Align both sides to the common ancestor — rejected; the result pane is what the user edits, so result-as-spine keeps connectors stable as the result changes. UI-side alignment — rejected, violates "no merge logic in frontend."

## R3 — Connector rendering that follows lines on scroll/resize

**Decision**: SVG overlay positioned over the gutters between panes; connectors anchor to CodeMirror line block geometry (`view.lineBlockAt` / `coordsAtPos`) and re-project on every scroll and resize. Render only connectors whose endpoints are within (viewport ± small margin).

**Rationale**: FR-004/FR-006 and SC-004 demand connectors that re-anchor with no perceptible drift; querying CodeMirror's live line geometry per frame keeps bands and connectors locked to content. Viewport culling satisfies SC-007 (1,000+ regions) and the "render only visible regions" edge case.

**Alternatives considered**: Canvas overlay — comparable, but SVG paths are simpler to style (the "beautiful" Bézier curves seen between panes) and debug. Fixed-position connectors recomputed on a timer — rejected, causes drift/lag (fails SC-004).

## R4 — Highlight decorations (line band + word span)

**Decision**: Drive CodeMirror 6 decorations from core payloads: line-level `Decoration.line` for the band per category (added/removed/modified/conflicting) and `Decoration.mark` for word spans within modified lines. Colors come from a theme token map, not literal names.

**Rationale**: FR-003/FR-011 require a line band plus distinct word-level marking; CodeMirror decoration sets update efficiently on apply/revert (FR-012). Distinct, consistent category colors satisfy FR-003 and SC-006. Vendor-neutral palette satisfies FR-017/Principle VI.

**Alternatives considered**: Inline DOM spans managed by hand — rejected, fights CodeMirror's virtualized rendering and breaks on scroll.

## R5 — Apply/revert as reversible operations

**Decision**: Model each apply/revert in `ops.rs` as a command recording the affected result line range and the before/after content, pushed onto an undo stack. Apply inserts a side's hunk content into the result; revert pops/undoes it; a global undo replays the inverse.

**Rationale**: FR-010 and Principle IV require symmetric, individually-reversible operations; a command/undo-stack model gives exact restoration (SC-005) and supports the constitution-mandated apply→revert round-trip test. Operations are scoped to one region so unrelated regions are untouched (FR-010, Edge Cases adjacent/overlapping).

**Alternatives considered**: Recompute result from a set of per-hunk "chosen side" flags — clean for non-overlapping hunks but loses manual center-pane edits (FR-012) and complicates undo ordering; rejected in favor of an explicit operation log.

## R6 — Core↔UI transport (Tauri IPC)

**Decision**: Tauri 2.x commands with `serde_json` payloads: `open_session` returns the full aligned model (panes, hunks, filler, word spans, status); `apply_change`/`revert_change`/`undo`/`navigate` return a delta (changed hunks + new status) the UI applies incrementally.

**Rationale**: Keeps all logic in Rust (constitution); deltas (not full re-serialization) keep apply/revert under SC-003's 100 ms and avoid re-rendering the whole document. Typed contracts documented in `contracts/`.

**Alternatives considered**: Expose core via WASM in the webview — rejected, duplicates the engine and blurs the "no merge logic in frontend" boundary; Tauri IPC keeps a single native engine.

## R7 — Whitespace handling

**Decision**: `mcr-core` exposes a whitespace mode (none / ignore-trailing / ignore-all) applied during tokenization in `diff.rs`; default `none`, user-toggleable from the UI which re-requests the session.

**Rationale**: FR-014 + Assumptions (default "do not ignore", toggleable). Doing it at tokenization keeps a single source of truth and avoids the UI reinterpreting diffs.

**Alternatives considered**: UI-side whitespace filtering — rejected (frontend logic). Post-diff filtering — messier than tokenizer-level normalization.

## Resolved unknowns

All Technical Context items are determined; no remaining NEEDS CLARIFICATION. Constitution-bound choices (Rust/Tauri/CodeMirror) were not re-litigated.
