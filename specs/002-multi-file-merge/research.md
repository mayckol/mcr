# Phase 0 Research: Multi-File Merge Navigator

All unknowns from the plan's Technical Context are resolved below. Each item gives the decision, the
rationale, and the alternatives rejected. The git-mechanics findings were established by inspecting
Git's `git-mergetool--lib` behavior and MCR's current launch path.

## R1 — How MCR obtains the full conflicted-file set

**Decision**: Self-discovery from inside the repository at launch. When MCR is invoked by
`git mergetool` (it receives one file's `LOCAL/BASE/REMOTE/MERGED`), it anchors the repo with
`git rev-parse --show-toplevel` (from the directory of the absolute `MERGED` path) and enumerates
the full unmerged set with:

```
git -C <root> diff --name-only --diff-filter=U --relative
```

For each conflicted path it reconstructs the three sides from the index stages rather than relying
on per-file temp files:

```
base     = git show :1:<path>     # stage 1 (common ancestor; may be absent → 2-way)
local    = git show :2:<path>     # stage 2 (ours / LOCAL)
incoming = git show :3:<path>     # stage 3 (theirs / REMOTE)
```

The one file Git materialized (the current `LOCAL/BASE/REMOTE`) is used directly for that entry; all
other entries are reconstructed from stages. Each entry's write-back target is its **worktree path**
(for the Git-passed file that is `$MERGED`; for the rest it is `<root>/<path>`).

**Rationale**: Git's mergetool contract physically passes only one file per spawn and conveys no
count, so the >1-vs-1 decision and the whole-set view cannot come from Git's arguments — MCR must
discover the set itself. Self-discovery keeps the documented `mergetool.mcr.cmd` config working with
**no wrapper script** (constitution III). Stage reconstruction (`git show :N:`) is exactly what Git
itself does to build the temp files, so it is faithful.

**Alternatives rejected**:
- *Repeated 4-tuples / `--merge-manifest` launcher*: requires a wrapper or non-standard config,
  violating constitution III ("no wrapper script beyond documented configuration").
- *Parsing `git status --porcelain`*: works but `diff-filter=U` / `ls-files -u` are narrower and
  give precisely the unmerged paths without status-code interpretation.

## R2 — Exit-code strategy reconciling one window with Git's per-file loop

**Decision**: MCR's exit code reflects **only the file Git handed this invocation** (the `$MERGED`
path). On exit:
- If that file is resolved → write it, exit `0`. Git (with `trustExitCode=true`) marks it resolved,
  `git add`s it, and advances its loop.
- If that file is *not* resolved (user skipped it) → exit non-zero → Git treats it as aborted.

Because MCR also stages every *other* file it resolved (R3), Git's next loop iterations find those
already resolved and skip them, and re-invoke MCR only for files still unmerged.

**Rationale**: This is the only model that (a) never lies to Git about the current file, (b) does not
require MCR to bypass or fake Git's loop, and (c) makes Git's own loop the resume driver. It matches
Git's documented behavior: the loop list is snapshotted up front, but each iteration re-checks
`ls-files -u` and skips paths already resolved.

**Alternatives rejected**:
- *Single aggregate exit for the whole batch from one invocation*: would require MCR to `git add`
  files Git didn't pass and then succeed/fail as a unit — desyncs Git's per-file "N paths resolved"
  accounting and is version-fragile.
- *Always exit 0 to avoid aborting*: would tell Git an unresolved file is resolved → silent data
  loss. Rejected outright (constitution IV).

**Residual risk surfaced**: with `trustExitCode=false`, Git falls back to an mtime check plus an
interactive "Was the merge successful?" prompt per file. MCR must ensure it actually changes
`$MERGED`'s content/mtime when resolved. The MCR setup docs already require `trustExitCode=true`;
the quickstart asserts it.

## R3 — Incremental persist + resume

**Decision**: The moment a file becomes resolved (via the editor reaching `fully_resolved`, or a
whole-file accept from the list), MCR writes its result to the worktree path and runs
`git -C <root> add -- <path>`. `save_merged` (already per-session) is the write primitive; staging
is a new step in a `save_and_stage(session_id)` helper. Resolved work therefore survives quitting:
a later `git mergetool` run (or Git's own continuing loop) re-discovers only the still-unmerged
files and resumes.

**Rationale**: Directly implements the clarified exit/persistence model (Clarifications 2026-06-30,
FR-017) and leverages Git's loop for resume with no extra bookkeeping. Staging-on-resolve is what
makes Git skip done files.

**Alternatives rejected**: *Write all files only at the end / hold in memory until finish* — loses
work on crash or early quit and reintroduces the all-or-nothing model the clarification removed.

## R4 — Backup / `.orig` machinery gap and reversibility

**Decision**: For files MCR writes+stages directly (every file except the one Git itself manages),
Git's `mergetool.keepBackup` / `.orig` / `keepTemporaries` handling does **not** apply. MCR writes
its own `<path>.orig` backup of the pre-resolution worktree content before overwriting, when the
user's `mergetool.keepBackup` is true (read via `git config --get mergetool.keepBackup`, default
true). The original conflicted blobs also remain recoverable from index stages until commit.

**Rationale**: Preserves constitution IV (every applied change reversible; no silent loss) and keeps
behavior consistent with what a user expects from `git mergetool`. Documented as a known difference
in quickstart.

**Alternatives rejected**: *Ignore the gap* — would make non-Git-passed files lose the `.orig` a
user may rely on; inconsistent and risky.

## R5 — Single-vs-multi decision and fallbacks

**Decision**: Let `N` = count from `git diff --name-only --diff-filter=U`. `N > 1` → show the
file-selection list as the entry point; `N == 1` → open that file directly in the three-pane editor
(no list). `N == 0` → nothing to resolve, exit cleanly. If `git rev-parse --show-toplevel` fails
(launched outside a worktree) or discovery errors, fall back to opening just the single file Git
passed, with no list — preserving today's behavior and the demo/standalone path.

**Rationale**: Matches FR-001/FR-015 and the clarified trigger exactly, and degrades safely when the
repo context is unavailable.

**Alternatives rejected**: *Always show the list even for one file* — contradicts the clarification
and adds a step for the common single-file case.

## R6 — UI: file list, view switching, lazy open, modal

**Decision**:
- **One editor, swap content.** Reuse the single `MergeEditor` instance and route every file switch
  through the existing `apply(model)` funnel in `main.ts`; do **not** instantiate a second editor
  (the decoration `viewHolder` is keyed globally per pane name and a second instance would clobber
  it).
- **File-list component**: a new `src/files/list.ts` class modeled on `ShortcutsPanel` (constructor
  takes a root element + select callback; `render()` builds rows; click selects and toggles an
  active-row class). Markup: `<aside id="file-list">` as a flex sibling of `#merge-container`.
- **Lazy open**: list all files up front from a lightweight summary (path + status), but open a
  file's `MergeSession`/editor model on first selection (`select_session`) so a 200+ file set does
  not load every editor (SC-003).
- **Exit-confirmation modal**: a new `src/confirm/modal.ts` class reusing the existing
  `.mcr-modal-*` CSS (same shape as `ShortcutsPanel`), gating `#save-exit` / `#abort` on unresolved
  files and listing the remaining ones (FR-008).
- **Client state**: add module-level `files` + `activeFile` in `main.ts`, updated alongside
  `apply()`. No store library (consistent with the current architecture).

**Rationale**: Follows the codebase's established patterns (component classes, single render funnel,
hand-rolled CSS, no framework), minimizing surface area and keeping merge logic in Rust.

**Alternatives rejected**: *One `MergeEditor` per file / a reactive framework / a state store* — all
fight the existing singleton + global-decoration design and add dependencies for no benefit.

## Cross-cutting decisions

- **Conflicted files only** in the list (Clarifications 2026-06-30); cleanly auto-merged files are
  excluded. Discovery uses `--diff-filter=U`, which yields exactly that set, including delete/modify,
  both-added, and binary conflicts.
- **Special conflicts** (binary, delete/modify) appear in the list with a distinct status and a
  per-file accept choice rather than the text three-pane editor (FR-002, FR-014).
- **The future comparison mode** (files/branches/commits, Principle V) can reuse the same list
  component backed by a different discovery source; kept in mind but out of scope.
