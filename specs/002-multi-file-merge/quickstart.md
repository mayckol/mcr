# Quickstart & Validation: Multi-File Merge Navigator

Runnable scenarios that prove the feature end-to-end. References [contracts](./contracts/tauri-commands.md)
and [data-model.md](./data-model.md) rather than duplicating shapes.

## Prerequisites

- MCR built and configured as the Git merge tool (see README "Use as a `git mergetool`"), with
  `mergetool.mcr.trustExitCode true` (required for the exit-code contract, research.md R2).
- A scratch Git repo with a branch that conflicts in **multiple** files.

## Scenario A — Multi-file conflict, resolve all in one window (US1+US2, P1)

```bash
# build a repo with conflicts in 3+ files
git init demo && cd demo
printf 'a1\n' > a.txt; printf 'b1\n' > b.txt; printf 'c1\n' > c.txt
git add . && git commit -m base
git switch -c feature
printf 'a-feature\n' > a.txt; printf 'b-feature\n' > b.txt; printf 'c-feature\n' > c.txt
git commit -am feature
git switch main
printf 'a-main\n' > a.txt; printf 'b-main\n' > b.txt; printf 'c-main\n' > c.txt
git commit -am main
git merge feature        # conflicts in a.txt, b.txt, c.txt
git mergetool            # launches MCR once for the first conflicted file
```

**Expected**:
1. MCR opens **one window** showing a **file list** of 3 conflicted files (not three separate
   windows) — FR-001, SC-001.
2. Each row shows path + status (unresolved); progress reads "0 of 3 resolved" — FR-002, FR-006.
3. Selecting `a.txt` opens it in the three-pane editor in the same window (< 300 ms) — FR-003,
   SC-003. Resolving it flips its row to resolved and progress to "1 of 3" — FR-005, FR-006.
4. After resolving all three, the session indicates fully resolved and offers a single finish action
   — FR-007. Finishing exits 0; `git mergetool` completes with all three files staged/resolved.

**Verify** (after finish): `git diff --name-only --diff-filter=U` prints nothing; `git status` shows
all three staged.

## Scenario B — Whole-file accept from the list + next-unresolved (US3, P2)

In Scenario A's window, from the list:
- Choose **accept local** on `a.txt` and **accept incoming** on `b.txt` *without opening the editor*
  — both flip to resolved with the expected side's content (FR-009). Each is staged immediately.
- Invoke **next unresolved** → focus lands on `c.txt` (FR-011).

**Verify**: `git show :2:a.txt`-side content is in `a.txt`'s worktree file; the accept is reversible
via undo before finish (FR-010).

## Scenario C — Partial resolve, exit with confirmation, resume (Clarifications, FR-008/FR-017)

1. Resolve only `a.txt`, leave `b.txt`/`c.txt` conflicted, click Save & Exit / Abort.
2. **Expected**: a confirmation modal lists the remaining unresolved files (`b.txt`, `c.txt`) before
   exiting — FR-008.
3. On confirm: `a.txt` stays resolved and staged (not discarded) — FR-017. `b.txt`/`c.txt` remain
   conflicted.

**Verify**: `git diff --name-only --diff-filter=U` lists only `b.txt c.txt`; `git status` shows
`a.txt` staged. Re-running `git mergetool` reopens MCR with only `b.txt`/`c.txt` (resume) — the
"re-run after partial" edge case.

## Scenario D — Single conflicted file opens directly (FR-015, SC-007)

Repeat with a merge that conflicts in only **one** file.

**Expected**: MCR opens straight into the three-pane editor with **no file-list step** — equivalent
to today's single-file behavior.

## Scenario E — Special conflicts (FR-014)

Create a delete/modify conflict (one side deletes `b.txt`, the other edits it) and a binary conflict.

**Expected**: both appear in the list with a distinct status and an **accept-only** per-file choice;
they do not open the text three-pane editor.

## Scenario F — Outside a worktree / discovery failure (R5 fallback)

Invoke MCR's binary with four file paths but **not** inside a Git repo (or with discovery failing).

**Expected**: MCR opens just the single file it was handed, with no list — no crash, no regression.

## Backup behavior (R4)

With `mergetool.keepBackup true` (default), after resolving non-Git-passed files, confirm a
`<path>.orig` backup exists for them (MCR writes it, since Git's own `.orig` machinery only covers
the file Git passed).

## Automated checks (CI gates)

- `cargo test -p mcr-core` — engine round-trip unaffected (apply→revert restores state, SC-005).
- `cargo test` in `src-tauri` — discovery (unmerged set, stage reconstruction), `finish`
  aggregation, exit-code selection.
- `npm --prefix ui run test` — `files_list` (rows, status, select, stable order) and `exit_modal`
  (lists unresolved, confirm/cancel) components in jsdom.
- `npm --prefix ui run typecheck` — wire types match.
- `bash scripts/vendor-neutral-check.sh` — no vendor reference in new UI strings (constitution VI).
- End-to-end: a scripted real `git mergetool` run over a multi-file conflict (constitution
  Development Workflow gate).
