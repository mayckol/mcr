# Feature Specification: Multi-File Merge Navigator

**Feature Branch**: `002-multi-file-merge`

**Created**: 2026-06-30

**Status**: Draft

**Input**: User description: "we must handle multiple file changes like jetbrains... sample [Image #5] we must use the same flow"

## Overview

Today MCR resolves one file at a time: Git's `mergetool` loop launches the app once per
conflicted file, each invocation showing a single three-pane editor and then closing. A merge
that touches twenty files means twenty separate windows opened in sequence, with no way to see
the whole set, jump between files, skip a hard one and come back, or track overall progress.

This feature introduces a **multi-file session**: a single window that lists every conflicted file
in the merge, shows each file's resolution status, lets the user move freely between files, and
opens any file into the *same* three-pane apply/revert editor that already exists. The existing
per-file resolution experience is unchanged — when the merge has more than one conflicted file it
is reached through a file-selection list shown as the session's entry point; when the merge has a
single conflicted file MCR opens it straight into the three-pane editor with no list step.

## Clarifications

### Session 2026-06-30

- Q: When the user resolves some but not all files and exits, what happens to completed work? → A: Incremental persist + resume — each file's resolution is written out and staged the moment it is resolved; exiting with files still unresolved first shows a confirmation modal listing the remaining unresolved files, and on confirm the resolved files are kept while the unresolved ones stay conflicted for a later `git mergetool` run to resume (no discard-all behavior).
- Q: Which files does the selection list manage in the mergetool flow? → A: Conflicted (unmerged) files only — including delete-modify, both-added, and binary conflicts; cleanly auto-merged files are excluded. A future compare feature (files / branches / commits, per constitution Principle V) will reuse this multi-file list as an all-changed-files mode.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See and navigate all conflicted files in one session (Priority: P1)

A user starts a merge that conflicts in several files and launches MCR. Instead of one isolated
file, they see a panel listing every conflicted file in the merge, each row showing the file's path
and a status (unresolved / resolved, plus a distinct marker for special conflicts like binary or
delete-modify). Selecting a file opens it in the three-pane merge editor in the same window. The
user works one file, then picks the next file from the list without the window closing and
reopening, and at all times can see how many files remain.

**Why this priority**: The whole request is "handle multiple file changes ... same flow." Seeing
the full set of files and moving between them in one window is the feature; without the list there
is no multi-file experience. The three-pane editor it opens into already exists, so this story is
the new value on its own.

**Independent Test**: Launch MCR on a merge with at least three conflicted files (including one
special conflict such as delete-modify or both-added). Verify a file list appears with one row per
conflicted file and a status per row, that selecting any row opens that file in the three-pane
editor within the same window, and that the user can return to the list and open a different file
without the session ending.

**Acceptance Scenarios**:

1. **Given** a merge with more than one conflicted file, **When** MCR opens for that merge, **Then**
   a file-selection list (the session's entry point) is shown first — one entry per file with its
   path and a per-file status indicator — before any file's editor opens.
2. **Given** the file list, **When** the user selects a file, **Then** that file opens in the
   existing three-pane merge editor within the same window, with the list still reachable.
3. **Given** a file open in the editor, **When** the user chooses another file from the list,
   **Then** the editor switches to that file without closing the session, preserving the first
   file's resolution work.
4. **Given** files with different states, **When** the list is shown, **Then** unresolved, resolved,
   and special-conflict (e.g. binary / delete-modify) files are visually distinguishable at a glance.
5. **Given** a merge with exactly one conflicted file, **When** MCR opens for that merge, **Then**
   that file opens directly in the three-pane editor with no file-selection list step shown.

---

### User Story 2 - Track per-file status and overall progress to completion (Priority: P1)

As the user resolves each file, that file's row in the list updates to "resolved," and a summary
shows remaining-vs-total (e.g. "3 of 8 resolved"). The user can tell at any moment which files are
done, which still have conflicts, and when the entire merge is fully resolved. When every file is
resolved, the session signals it is complete and can be finished in one action; if the user leaves
files unresolved, finishing communicates clearly that conflicts remain.

**Why this priority**: A multi-file list without live status is just a launcher. Knowing what is
done, what is left, and when the whole merge is finished is what turns the list into a workflow,
and it is what lets the user finish a many-file merge confidently. It pairs with Story 1 to form
the MVP.

**Independent Test**: Open a multi-file session, resolve one conflicted file, and confirm its row
flips to "resolved" and the progress summary decrements the remaining count. Resolve the rest and
confirm the session reports fully-resolved; leave one unresolved and confirm finishing surfaces the
remaining conflict rather than silently completing.

**Acceptance Scenarios**:

1. **Given** a file with conflicts, **When** the user resolves it, **Then** its list row changes to
   a resolved state and the overall progress count updates immediately.
2. **Given** a session with remaining conflicts, **When** the user views the summary, **Then** it
   shows how many files are resolved out of the total and how many conflicts remain.
3. **Given** all files resolved, **When** the user views the session, **Then** it indicates the
   merge is fully resolved and offers a single action to finish.
4. **Given** one or more files still conflicted, **When** the user attempts to finish the session,
   **Then** the system surfaces the remaining unresolved files instead of completing silently.

---

### User Story 3 - Resolve whole files from the list and move quickly between them (Priority: P2)

For files that do not need line-by-line attention, the user acts on the whole file directly from
the list: accept the local version or accept the incoming version — without opening the editor. The
user can also jump to the next unresolved file with one
action ("next conflict") to move through the set efficiently, and the list keeps a stable order so
their place is predictable.

**Why this priority**: This is the speed layer of the JetBrains-style flow — bulk per-file accept
and "next unresolved" let a user clear a large merge fast. It is highly valuable but the session is
already usable for resolution through Story 1 + 2, so it ranks just below the core.

**Independent Test**: In a multi-file session, accept the local side of one file and the incoming
side of another directly from the list (without opening the editor), confirm both flip to resolved
with the expected content, then use "next unresolved" to land on the remaining conflicted file.

**Acceptance Scenarios**:

1. **Given** a file in the list, **When** the user chooses "accept local" or "accept incoming" for
   that file, **Then** the file is resolved to that side and marked resolved without opening the
   editor.
2. **Given** a per-file whole-file accept, **When** it completes, **Then** it is reversible to the
   file's prior unresolved state, consistent with the editor's non-destructive apply/revert.
3. **Given** a session with several unresolved files, **When** the user invokes "next unresolved,"
   **Then** focus moves to the next unresolved file in list order.
4. **Given** the file list, **When** files change state during the session, **Then** the list's
   ordering stays stable so the user does not lose their place.

---

### Edge Cases

- A merge with exactly one conflicted file — MCR opens that file straight into the three-pane
  editor with no file-selection list step (no regression versus today's single-file experience).
- The user exits with files still unresolved — a confirmation modal lists the remaining unresolved
  files before exit; on confirm, already-resolved files are kept (each was written out and staged
  when resolved) and the unresolved files stay conflicted for a later run to resume. No resolved
  work is discarded.
- Re-running the merge after a session that resolved only some files — files already resolved in a
  prior run are not re-presented as conflicted (they appear resolved or are excluded from the
  remaining set), so the user is not asked to redo completed work.
- A file the merge changed cleanly on only one side (auto-merged, no conflict) — excluded from the
  list, since the mergetool flow only manages conflicted files.
- A very large conflict set (hundreds of files) — the list stays responsive and scrollable, and
  selecting a file opens it without loading every file's editor up front.
- A file deleted on one side and modified on the other (delete/modify conflict) — represented in
  the list with a status the user can act on, not silently dropped.
- Binary or unreadable files in the changeset — listed but clearly marked as not resolvable in the
  text three-pane editor, with a per-file choice instead.
- The user finishes with files left unresolved — the session communicates which files remain rather
  than reporting success.
- The user reopens an already-resolved file and edits it — its status reflects the new state (can
  return to unresolved if they reintroduce a conflict).
- Switching away from a file with in-progress edits — that file's work is preserved when the user
  returns to it within the session.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: When the merge being resolved has more than one conflicted file, MCR MUST present a
  single session window whose file-selection list (one entry per file the session manages) is the
  entry point the user picks a file from, shown before any individual file's editor opens.
- **FR-002**: Each file entry MUST show the file's path and a per-file status of at least:
  unresolved (conflicted), resolved, and a distinct special-conflict marker (e.g. binary or
  delete-modify) for conflicts not resolvable in the text three-pane editor.
- **FR-003**: Selecting a file from the list MUST open that file in the existing three-pane
  apply/revert merge editor within the same session window, without starting a separate process or
  closing the session.
- **FR-004**: The user MUST be able to switch between files freely (in any order, repeatedly)
  without the session ending, and a file's in-progress resolution MUST be preserved when the user
  navigates away and back within the session.
- **FR-005**: The session MUST update a file's status live as it becomes resolved or returns to
  unresolved (including when the user manually edits a previously-resolved file).
- **FR-006**: The session MUST display overall progress: number of files resolved out of total and
  the number of conflicts remaining.
- **FR-007**: The session MUST indicate when all files are resolved and offer a single action to
  finish/complete the whole merge.
- **FR-008**: When the user attempts to finish or exit with files still unresolved, the session MUST
  show a confirmation modal listing the remaining unresolved files instead of completing silently;
  confirming exits while keeping the resolved files and leaving the unresolved ones conflicted for a
  later run to resume.
- **FR-009**: The user MUST be able to resolve a whole file directly from the list by accepting the
  local side or the incoming side, without opening the three-pane editor.
- **FR-010**: Whole-file accept actions taken from the list MUST be reversible to the file's prior
  state, consistent with the constitution's non-destructive, reversible apply/revert guarantee.
- **FR-011**: The user MUST be able to jump to the next unresolved file with a single action.
- **FR-012**: The file list MUST keep a stable, predictable order as file states change during the
  session, so the user does not lose their place.
- **FR-013**: Each resolved file MUST be written out as its own result, preserving today's per-file
  write-out semantics (the user's resolution for a file is the artifact written back for that file).
- **FR-014**: Files that cannot be resolved in the text three-pane editor (e.g. binary or deletion
  conflicts) MUST still appear in the list with a clearly distinct status and a per-file resolution
  choice.
- **FR-015**: When the merge being resolved has exactly one conflicted file, MCR MUST open that file
  straight into the three-pane editor with no file-selection list step, preserving today's
  single-file behavior with no loss of capability.
- **FR-016**: No user-facing text, label, or asset in the file list or session UI may name or imply
  the third-party IDE whose interaction model inspired this experience (per constitution
  Principle VI).
- **FR-017**: Each file's resolution MUST be persisted (written out and staged) at the moment it is
  resolved, so completed work survives quitting and a later `git mergetool` run resumes only the
  still-unresolved files. The session reports overall success to the surrounding Git command only
  when every targeted file was resolved; exiting with unresolved files (after the FR-008
  confirmation) keeps the resolved files and leaves the unresolved ones conflicted — it MUST NOT
  discard already-resolved work.

### Key Entities *(include if feature involves data)*

- **Merge Session (multi-file)**: The whole set of conflicted files being resolved in one window;
  holds the ordered list of file entries and the overall progress/completion state. Extends the
  existing single-file Merge Session from being one file to being a collection of them.
- **File Entry**: One conflicted file in the session; has a path, a conflict category (text conflict
  / special such as binary or delete-modify), a per-file resolved/unresolved status, and a reference
  to its own (lazily-opened) three-pane merge state.
- **Session Progress**: The derived summary over all file entries — total files, resolved count,
  remaining-conflict count, and overall complete/incomplete state.
- **Whole-File Action**: A reversible per-file resolution chosen from the list (accept local /
  accept incoming) that resolves a file without opening the editor, recorded so it can be undone.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can resolve a merge that changed N files end-to-end within a single window
  session, without the application opening and closing once per file.
- **SC-002**: From the session, a user can identify at a glance, for every file, whether it is
  unresolved, resolved, or a special conflict (binary / delete-modify), and how many conflicts
  remain overall.
- **SC-003**: Selecting a file from the list reveals its three-pane editor within 300 ms for a
  changeset of up to 200 files, and the list itself stays scrollable and responsive at that size.
- **SC-004**: A user can clear a merge of many simple conflicts by accepting whole files (local or
  incoming) directly from the list, without opening the editor, reaching fully-resolved in fewer
  steps than opening each file individually.
- **SC-005**: Switching between files never loses a file's in-progress resolution: returning to a
  file restores exactly the state the user left it in, in 100% of in-session switches.
- **SC-006**: Attempting to finish or exit with unresolved files shows a confirmation modal listing
  the remaining files 100% of the time (no silent completion of an incomplete merge), and resolved
  files persist across the exit in 100% of cases.
- **SC-007**: For a single-file merge, the session behaves equivalently to today's single-file
  editor with no added steps to resolve and finish.

## Assumptions

- **Primary scope is merge-conflict resolution.** The "multiple file changes" being handled are the
  merge's conflicted files. A planned follow-up will reuse this same multi-file list for read-only
  comparison of files / branches / commits (constitution Principle V) as an all-changed-files mode;
  that comparison capability is out of scope for this spec.
- **The trigger counts the merge's conflicted files.** In the Git mergetool flow the files the
  session manages are the merge's conflicted (unmerged) files — the set Git's mergetool itself
  operates on. The list-vs-direct decision is therefore: more than one conflicted file → show the
  selection list; exactly one → open the editor directly. Cleanly auto-merged (non-conflicting)
  files are excluded from the list (Clarifications, Session 2026-06-30).
- **MCR must self-discover the conflicted set at launch.** Verification of Git's actual mergetool
  contract established a non-obvious constraint: `git mergetool` spawns the tool **once per
  conflicted file**, sequentially and blocking, and never conveys a file count or the whole set in
  any single call. So the >1-vs-1 decision cannot come from Git's invocation arguments — MCR must
  discover the full conflicted set itself from inside the repository at launch (e.g. via Git
  plumbing). The exact discovery mechanism is a plan-phase decision; the user-facing rule above is
  fixed regardless of mechanism. When MCR is launched outside a Git worktree (or discovery fails),
  it falls back to opening the single file it was handed, with no list.
- **One window covers the whole set; resolution persists incrementally.** Because Git won't hand
  over the set across its per-file loop, a single MCR window presents every targeted file, and each
  file is written back and staged the moment it is resolved (FR-017). The user may resolve all in
  one sitting or quit early (after the FR-008 confirmation) and resume the rest on a later
  `git mergetool` run. A consequence the plan must acknowledge: for files MCR writes/stages directly
  (rather than via Git's per-file machinery), Git's per-file backup/temp behavior
  (`mergetool.keepBackup`, `keepTemporaries`, `.orig` files) does not apply to those files; this is
  a user-visible behavior difference to document and reconcile during planning.
- **The per-file editor is the existing three-pane editor.** This feature adds the file list,
  navigation, status, and progress around it; it does not change how an individual file is resolved
  (panes, highlights, connectors, apply/revert), which is covered by feature 001.
- **"Same flow" means the JetBrains-style multi-file merge layout** — a list/changeset of affected
  files with per-file status that opens into the three-pane editor — reproducing the *interaction
  idea* only, never any third-party assets, code, or branding, per the constitution.
- **Per-file write-out is preserved.** Each file's resolved result is written back as that file's
  output, matching today's `MERGED`-per-file contract; finishing the session does not change what a
  resolved file writes.
- **Stable list order** defaults to the order files are discovered (e.g. path order) and does not
  reorder as statuses change, to keep the user's place predictable.
- **List opening is lazy.** Files are listed up front but their editors are opened on demand, so a
  large changeset does not pay the cost of fully loading every file at launch.
