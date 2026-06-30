# Changelog

## v0.1.0

Initial release.

- Three-pane visual merge editor: local / editable result / incoming, with
  line-band + gutter highlights, word-level intra-line spans, full-line connector
  ribbons, synced scrolling, and per-change apply/revert gizmos.
- Rust core engine: three-way diff3, alignment with filler, reversible
  apply/revert/undo/redo, whitespace modes.
- Configurable keyboard shortcuts (Cmd+Z / Cmd+Shift+Z by default) with a rebind
  panel.
- Tokyo Night theme.
- macOS (.dmg) and Linux (.AppImage/.deb) bundles; curl installer and Homebrew
  cask.
