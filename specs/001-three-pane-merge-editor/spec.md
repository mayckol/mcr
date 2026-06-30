# Feature Specification: Three-Pane Visual Merge Editor

**Feature Branch**: `001-three-pane-merge-editor`

**Created**: 2026-06-30

**Status**: Draft

**Input**: User description: "I need a merge editor with the same approach from jetbrains, 3 panes, beautiful style to trace the changes and apply / revert. We must have the highlight with the exact style binding and following the lines properly"

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Resolve a conflicted file in three panes (Priority: P1)

A user opens a file that has merge conflicts. They see three side-by-side panes: their side on the left, the merge result in the center, and the incoming side on the right. Each changed region is highlighted with a colored band, and a connecting line traces every changed region from a side pane across to the matching lines in the center result pane. The user reads each change in context and works through the file top to bottom until the center pane holds the resolution they want.

**Why this priority**: This is the core value of the feature — a user cannot resolve a conflict without seeing all three versions aligned and understanding which lines changed on each side. Without it there is no product.

**Independent Test**: Open a file containing at least one conflict and one non-conflicting change on each side. Verify all three panes render, every changed region is highlighted, and each highlight is visually connected by a line to its corresponding region in the center pane. This delivers a readable, navigable conflict view on its own.

**Acceptance Scenarios**:

1. **Given** a file with changes on both sides, **When** the merge editor opens, **Then** three panes are shown (left side, center result, right side) with the center result editable and the two side panes read-only.
2. **Given** a changed region on a side pane, **When** the user views it, **Then** the region is highlighted with a colored band and a connecting line links it to the aligned lines in the center pane.
3. **Given** lines that exist on one side but not the other, **When** the panes are aligned, **Then** filler/blank space is inserted so corresponding lines stay horizontally aligned across panes.

---

### User Story 2 - Apply or revert a change with one action (Priority: P1)

For any highlighted change, the user clicks an apply control to accept that side's version into the center result, or a revert control to remove a change that was already applied. The center result updates immediately and the highlight/connecting line state reflects the new result. The user can apply a change from the left, apply a change from the right, and undo either action without losing other work.

**Why this priority**: Tracing changes is only useful if the user can act on them. Apply/revert per change is the second half of the core loop and is required for an MVP.

**Independent Test**: With a conflicted file open, apply one change from the left and one from the right, then revert one of them. Verify the center result reflects each action immediately and that reverting restores the prior state of only that change. This is testable end to end without any other feature.

**Acceptance Scenarios**:

1. **Given** a highlighted change on a side pane, **When** the user activates its apply control, **Then** that side's content for the region is inserted into the center result and the region is marked resolved.
2. **Given** an applied change, **When** the user activates its revert control, **Then** the applied content is removed from the center result and the change returns to its unresolved state.
3. **Given** any apply or revert action, **When** it completes, **Then** the action is reversible via a global undo without affecting unrelated changes.
4. **Given** a non-conflicting change on one side, **When** the user chooses "apply all non-conflicting from this side", **Then** every non-conflicting region from that side is applied in one action.

---

### User Story 3 - Precise highlight binding that follows the lines (Priority: P2)

Within a changed region, the user sees word-level (intra-line) highlighting that marks exactly which words changed, distinct from the line-level band. As the user scrolls, the highlight bands and the connecting lines stay locked to their lines: panes scroll in sync, connectors re-anchor to the correct on-screen line positions, and the styling (colors for added / removed / modified / conflicting) stays consistent. Highlights never drift away from the content they describe.

**Why this priority**: The user explicitly asked for "exact style binding" and connectors that "follow the lines properly." This precision is what makes the editor trustworthy and pleasant, but the editor is still usable for basic resolution without it, so it ranks below the core loop.

**Independent Test**: Open a file with modified lines (not just full-line add/remove), scroll through it, and resize the window. Verify word-level highlights mark the exact changed spans, connectors stay anchored to their lines through scrolling and resizing, and panes remain scroll-synced. Delivers the "beautiful, precise" tracing experience on its own.

**Acceptance Scenarios**:

1. **Given** a line modified on one side, **When** it is displayed, **Then** the specific changed words are highlighted within the line in addition to the line-level band.
2. **Given** the user scrolls any pane, **When** the view moves, **Then** all three panes scroll in sync and connecting lines re-anchor to the correct line positions without lag or drift.
3. **Given** added, removed, modified, and conflicting regions, **When** displayed, **Then** each category uses a distinct, consistent color treatment.
4. **Given** the window is resized, **When** the layout reflows, **Then** highlights and connectors realign to their lines.

---

### Edge Cases

- A region changed on both sides (true conflict) — both side highlights connect to the same center region and the user must choose; "apply non-conflicting" skips it.
- Adjacent or overlapping changes — each remains independently applicable/revertible without corrupting neighbors.
- Whitespace-only or end-of-line-only differences — surfaced according to the active "ignore whitespace" setting.
- Very large files / many changes — the view stays responsive and connectors render only for visible regions.
- A file with no remaining conflicts — the editor indicates the file is fully resolved and ready to be written out.
- The user manually edits the center pane inside a changed region — the change's resolved/unresolved state updates to reflect the manual edit.
- One side deletes lines the other modifies — alignment fillers and connectors still pair the regions correctly.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The editor MUST present three vertically-split panes simultaneously: the local side, the editable merged result in the center, and the incoming side.
- **FR-002**: The two side panes MUST be read-only and the center result pane MUST be editable.
- **FR-003**: The editor MUST highlight every changed region with a line-level colored band, using distinct, consistent colors for added, removed, modified, and conflicting regions.
- **FR-004**: The editor MUST draw a connecting line (gutter connector) from each side-pane change to its corresponding region in the center result pane.
- **FR-005**: The editor MUST insert alignment filler so that corresponding lines across the three panes stay horizontally aligned even when line counts differ.
- **FR-006**: The editor MUST keep all three panes scroll-synchronized, and connectors MUST re-anchor to correct line positions during scrolling and window resizing without visible drift.
- **FR-007**: Each changed region MUST expose an apply control that inserts that side's content into the center result.
- **FR-008**: Each applied region MUST expose a revert control that removes the applied content and returns the region to its unresolved state.
- **FR-009**: The editor MUST provide bulk actions to apply all non-conflicting changes from the left, from the right, and from both.
- **FR-010**: Every apply and revert action MUST be individually reversible via undo without affecting unrelated regions (non-destructive, symmetric apply/revert).
- **FR-011**: The editor MUST display word-level (intra-line) highlighting marking the exact changed spans within a modified line, visually distinct from the line-level band.
- **FR-012**: The editor MUST update the resolved/unresolved status of each region live as the user applies, reverts, or manually edits the center result.
- **FR-013**: The editor MUST indicate the overall resolution status of the file (e.g., remaining conflict count and a fully-resolved state).
- **FR-014**: The editor MUST offer a whitespace-handling setting that lets the user ignore whitespace-only differences when detecting changes.
- **FR-015**: Inspection and comparison MUST be read-only and MUST NOT modify the working file until the user explicitly writes out the result.
- **FR-016**: The editor MUST let the user navigate between changes (next/previous change) and reveal the targeted region in all three panes.
- **FR-017**: No user-facing text, label, color name, or asset may name or imply the third-party IDE whose interaction model inspired this experience.

### Key Entities *(include if feature involves data)*

- **Merge Session**: One file being merged; holds the three source versions (local, base/ancestor, incoming), the working merged result, and the overall resolution status.
- **Change Region (Hunk)**: A contiguous block of lines that differs; has an origin (local / incoming / both), a category (added / removed / modified / conflicting), a resolved/unresolved state, and references to its aligned line ranges in each pane.
- **Intra-line Span**: The specific word/character range within a changed line that differs, used for word-level highlighting.
- **Connector**: The visual link binding a change region in a side pane to its aligned region in the center pane; anchored to live line positions.
- **Apply/Revert Action**: A reversible operation recording what content was inserted or removed for one region, enabling symmetric undo.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can resolve a file with mixed conflicting and non-conflicting changes using only the three-pane view and the apply/revert controls, without editing raw conflict markers by hand.
- **SC-002**: Every changed region is visibly highlighted and connected to its center-pane counterpart; 100% of on-screen changed regions have a visible connector.
- **SC-003**: Applying or reverting a change updates the center result and is reflected on screen within 100 ms for files up to 5,000 lines.
- **SC-004**: While scrolling or resizing, connectors and highlights stay anchored to their lines with no perceptible drift (no user-visible misalignment between a highlight and its content).
- **SC-005**: Any apply or revert can be undone, restoring the exact prior state of that region in at least 99% of operations across a representative test suite.
- **SC-006**: In usability testing, at least 90% of users correctly identify, on first attempt, which words changed within a modified line (validating word-level highlight clarity).
- **SC-007**: The view remains interactive (scroll, navigate, apply) without stutter on files with at least 1,000 change regions.

## Assumptions

- The center result is the artifact the user ultimately writes back; the two side panes are reference-only.
- The three input versions (local, ancestor, incoming) are available to the session; producing them (e.g., via Git plumbing) is handled outside this feature.
- "Beautiful style / exact binding" refers to a polished IDE-style three-pane merge interaction (colored bands, gutter connectors, word-level highlights) — reproducing the *interaction idea* only, never any third-party assets, code, or branding, per the project constitution.
- Default change categories are added, removed, modified, and conflicting; their exact color palette is a design decision deferred to the design phase but must be distinct and consistent.
- Whitespace handling defaults to "do not ignore" and is user-toggleable.
- This spec covers the merge-editor experience (panes, highlighting, apply/revert, navigation); the Git mergetool entry contract and file I/O are covered by separate features.
