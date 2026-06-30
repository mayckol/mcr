# Changelog

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
