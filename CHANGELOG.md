# Changelog

## v0.3.6

- **The window opens instantly, on any repository size.** Repository discovery
  used to run before the window was even created, and merge mode then built one
  session per conflicted file (several git calls + two diffs each) on the main
  thread — on big repositories MCR looked hung or never-opened. Discovery now
  runs after the window is up (with a "Scanning repository…" status), the
  conflicted set is listed from a single index-only `git ls-files -u`, and merge
  sessions build lazily on first selection, as compare mode already did.
- **The UI never freezes on git work.** Every command now runs off the main
  thread, and manual edits in the result pane are debounced into one backend
  round-trip instead of shipping the full document on every keystroke.
- **Launched from another app, MCR takes focus.** Spawned as a child process
  (e.g. by fftracking), the window could open behind the caller and look like it
  never launched.
- **Linux launches are FUSE-free and fast.** The installer extracts the AppImage
  once and points every launcher at the extracted `AppRun` — no libfuse2
  requirement and no per-launch mount/self-extraction, including for external
  callers that exec MCR directly.

## v0.3.5

- **Change navigation skips resolved changes.** The next/previous arrows now stop
  only on unresolved changes, jumping over the dotted ghosts of already-applied
  ones. File-crossing and edge focus land on actionable changes too.
- **Linux mergetool opens without libfuse2.** The AppImage now self-extracts when
  libfuse2 is missing (common on newer distros), so `git mergetool` no longer
  silently fails to open a window.
- **No more terminal prompts on merge.** The installer sets
  `mergetool.prompt=false`, so git skips its "Hit return…" / "Was the merge
  successful?" questions — the MCR window opens straight away.

## v0.3.4

- **Change bands reach the pane edge.** Each pane's vertical scrollbar now sits on
  its outer edge, so a change highlight is never cropped by the bar and the
  scrollbar sits within the highlight. Connector ribbons anchor to the band edge.
- **Only the changed side is highlighted.** An edit made on one side no longer
  paints a connector-less band on the unchanged opposite side.
- **Accepted changes read as a ghost.** A resolved change drops its fill and shows
  a full-width dotted outline where it was, with the apply arrow removed.
- **Configurable editor font.** Settings → Editor picks the code font family,
  weight, and size; it applies live to all three panes and is remembered.

## v0.3.3

- **New logo.** Refreshed the app brand image.

## v0.3.2

- **Compare opens instantly on big diffs.** File sessions now build lazily on
  first selection instead of paying one `git show` + diff + full model build per
  changed file at launch; list/progress refreshes poll a cheap status counter
  instead of rebuilding every open session's model. A 300-file diff that took
  seconds now paints immediately.

## v0.3.1

- **Compare redesigned**: `mcr diff <branch|commit>` now compares one ref
  against your **working tree**, side by side — the ref on the left, your
  editable current file on the right. Hunks start unresolved (the right pane
  opens as your file, untouched); the `»` gizmos pull the ref's changes into
  your code. Save writes the working tree, never stages. The old two-ref form
  (`mcr diff A B`) is gone; the second positional is now the optional repo dir
  anchor. File statuses read ref → worktree (A = only in your tree, D = only at
  the ref).
- **File tree**: the sidebar groups files into collapsible folders, IDE-style —
  single-child directory chains compress into one node (`a/b/c`), folders show
  their file count, rows show basenames (full path on hover).
- **Change navigation crosses files**: `↓` on a file's last change continues at
  the first change of the next file (wrapping around the list); `↑` mirrors it
  backwards, landing on the previous file's last change.

## v0.3.0

- **Compare mode**: `mcr diff <refA> <refB>` opens a three-pane view — refA |
  working tree | refB. Cherry-pick hunks from either ref or edit freely; **Save**
  writes the working-tree file (never stages), **Close** exits cleanly. Bad
  arguments or refs exit 2 with usage on stderr, so editors/IDEs can script it.
  The sidebar lists changed files with A/M/D/R badges; the Linux launcher and a
  new macOS `mcr` shim wire the command up at install time.
- **Themes**: Tokyo Night (default), Tokyo Storm, Daylight, and Ember. One palette
  drives the whole app — chrome, editor, syntax highlighting, change bands,
  connectors. Pick in Settings (toolbar gear, Cmd/Ctrl+comma, or the macOS
  app-menu **Settings…**); the choice applies live and is remembered.
- Visual refresh: segmented toolbar groups, softer borders, tighter pane gap,
  change bands run flush to the pane edge, calmer focus highlight.
- **Merge safety fixes**:
  - Cancel in a multi-file merge staged resolved files and exited 0 — the exact
    opposite of aborting. It now confirms and exits non-zero without saving.
  - Finishing a merge rewrote binary conflicts with lossy text and resurrected
    files whose deletion was accepted.
  - macOS Cmd+Q exited 0 with unresolved conflicts, letting Git stage them.
  - Non-UTF8 (Latin-1 etc.) files were silently corrupted; they now resolve via
    raw-blob accept like binaries.
  - Saving flipped CRLF working-tree files to LF; line endings are preserved.
- macOS installer actually works now (`hdiutil` mount parse; correct bundle
  executable path in the mergetool shim).
- Windows: git calls no longer flash console windows; CI runs the app backend
  test suite on all three OSes.

## v0.2.1

- Bottom action bar: **Accept Left** / **Accept Right** apply all non-conflicting
  changes from a side; **Apply** writes the result and finishes, **Cancel** aborts.
- Semantic band colors — green = a change you can safely accept, yellow = a
  conflict you must pick a side for, red = a deletion; gutter and content tint now
  share one source so a row never stripes gutter-vs-content.
- Opening a file jumps the result pane to its first change and focuses the editor.
- Hunk gizmos fire on mousedown (the layer is rebuilt on every geometry refresh);
  apply arrows hide once a side is applied; the gizmo stays inside the visible band.
- `scrollbar-gutter: stable` keeps content width steady as the scrollbar appears.
- `git mergetool` registration: the installer writes an absolute-path foreground
  shim and registers `merge.tool=mcr` (without clobbering an existing tool).
- Tokyo Night: enkia keyword purple and teal/cyan type + property accents.

## v0.2.0

- Multi-file merge: `git mergetool` now opens one window listing every conflicted
  file, with per-file status and overall progress. More than one conflict shows a
  file list; a single conflict opens straight into the editor.
- Resolve a whole file from the list with **Ours** / **Theirs**, jump to the next
  unresolved file, and finish the whole merge from one place. Each file is written
  and staged the moment it is resolved; exiting with conflicts left confirms first
  and never stages unresolved content.
- Same-line conflicts can keep **both** sides: accept one, then append the other.
- The result pane is now freely editable, with undo/redo covering manual edits.

## v0.1.0

Initial release.

- Three-pane visual merge editor: local / editable result / incoming, with
  line-band + gutter highlights, word-level intra-line spans, full-line connector
  ribbons, synced scrolling, and per-change apply/revert gizmos.
- Rust core engine: three-way diff3, alignment with filler, reversible
  apply/revert/undo/redo, whitespace modes.
- Drop-in `git mergetool`: opens Git's LOCAL/BASE/REMOTE files, writes MERGED on
  Save & Exit (exit 0) or aborts (non-zero).
- Configurable keyboard shortcuts (Cmd+Z / Cmd+Shift+Z by default) with a rebind
  panel.
- Tokyo Night theme.
- macOS (.dmg) and Linux (.AppImage/.deb) bundles; curl installer and Homebrew
  cask.
