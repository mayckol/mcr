<p align="center">
  <img src="ui/public/logo.png" alt="MCR" width="320" />
</p>

# MCR

A **three-pane visual merge editor** — see the two diverging sides and the merged
result side by side, trace every change with line-locked highlights and connector
ribbons, and apply or revert each change individually. Built with a Rust core
(diff / alignment / reversible apply-revert) and a Tauri + CodeMirror UI.

- **Three panes**: local · editable result · incoming.
- **Trace changes**: line bands, word-level highlights, and curved connectors
  that bind each side change to its result region and follow the lines as you scroll.
- **Apply / revert**: per-change gizmos, bulk "apply all non-conflicting", and
  fully reversible undo/redo.
- **Compare branches & commits**: `mcr diff main feature` opens any two refs
  against your working tree — cherry-pick hunks from either side.
- **Configurable keyboard shortcuts** (Cmd+Z / Cmd+Shift+Z by default).
- **Themes**: Tokyo Night (default), Tokyo Storm, Daylight, and Ember — switch
  in Settings, applies live.

---

## Install

### macOS (Apple Silicon)

**Homebrew** (recommended):

```bash
brew install --cask mayckol/tap/mcr
```

**curl**:

```bash
curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh
```

Installs `MCR.app` to `/Applications`. The build is unsigned, so on the **first
launch** right-click the app → **Open** (Gatekeeper blocks a normal double-click
once). The installer strips the quarantine flag, so subsequent launches are normal.

### Linux (x86_64)

```bash
curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh
```

Installs the AppImage to `~/.local/bin/mcr.AppImage` with a `mcr` launcher and an
application-menu entry + icon. If `~/.local/bin` isn't on your `PATH`, the
installer tells you to add it.

Change the install prefix with `MCR_PREFIX`:

```bash
curl -fsSL .../install.sh | MCR_PREFIX=/usr/local sh
```

### Pin a specific version

```bash
curl -fsSL .../install.sh | MCR_VERSION=v0.1.0 sh
```

### Requirements

- **macOS**: 11 (Big Sur) or later, Apple Silicon.
- **Linux**: x86_64 with a WebKitGTK runtime (`libwebkit2gtk-4.1`), present on
  most modern desktops.

---

## Uninstall

**curl** (macOS and Linux):

```bash
curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh -s -- --uninstall
```

Removes `MCR.app` (macOS) or the AppImage + launcher + desktop entry (Linux).

**Homebrew** (macOS):

```bash
brew uninstall --cask mcr
```

---

## Usage

Launch **MCR** from the Applications menu / Launchpad, or from a terminal:

```bash
mcr              # macOS and Linux (installed by the installer)
open -a MCR      # macOS alternative
```

### The window

| Pane | Meaning |
|------|---------|
| **Local** (left) | Your side — read-only |
| **Result** (center) | The merged output — editable; this is what you keep |
| **Incoming** (right) | The other side — read-only |

Every changed region is highlighted and connected by a ribbon to its matching
lines in the result pane. Colors distinguish **added**, **removed**, **modified**,
and **conflicting** regions. The panes scroll together and the connectors stay
locked to their lines.

### Resolving changes

- **Apply a change** — click the `»` gizmo on the left to take the local version,
  or `«` on the right to take the incoming version. The result updates instantly.
- **Revert a change** — click the `×` gizmo on the applied region to undo it.
- **Bulk apply** — the toolbar `» Left` / `All` / `Right «` buttons apply every
  non-conflicting change from that side at once (conflicts are skipped).
- **Edit directly** — type in the center result pane; the region's state updates
  to "manually edited".
- **Navigate** — `↑` / `↓` jump to the previous / next change.
- **Undo / Redo** — every apply, revert, and bulk action is reversible.
- **Status** — the top-right shows how many conflicts remain (and "Resolved" when
  the file is fully merged).

### Whitespace

The **Whitespace** dropdown toggles how differences are detected: *Do not ignore*
(default), *Ignore trailing*, or *Ignore all*. Whitespace-only changes disappear
when ignored.

### Keyboard shortcuts

Defaults (⌘ = Cmd on macOS, Ctrl elsewhere):

| Action | Shortcut |
|--------|----------|
| Undo | `⌘Z` |
| Redo | `⌘⇧Z` |
| Apply all non-conflicting | `⌘⌥A` |
| Apply all from left / right | `⌘⌥←` / `⌘⌥→` |
| Next / previous change | `⌥↓` / `⌥↑` |

Click the **⌨** toolbar button to open the shortcuts panel: click any shortcut,
press the new keys to rebind it, `Esc` cancels, **Reset to defaults** restores
them. Bindings are saved and persist across restarts.

### Themes

Open **Settings** (the ⚙ toolbar button, `⌘,` / `Ctrl+,`, or **mcr-app →
Settings…** in the macOS menu bar) → **Appearance**. Four themes ship: **Tokyo
Night** (default), **Tokyo Storm**, **Daylight** (light), and **Ember**. The
choice applies immediately — editor, syntax colors, diff bands, everything — and
is remembered across launches.

### Compare branches & commits

`mcr diff` opens a three-pane comparison of any two refs — branches, tags, or
commit SHAs — with your **working tree in the middle**:

```bash
cd your-repo
mcr diff main feature          # two branches
mcr diff v1.2.0 v1.3.0         # two tags
mcr diff HEAD~3 HEAD           # two commits
mcr diff abc1234 my-branch     # mix freely
```

| Pane | Meaning |
|------|---------|
| **Left** | The file at the first ref — read-only |
| **Center** | Your current working-tree file — editable |
| **Right** | The file at the second ref — read-only |

The sidebar lists every file that differs between the two refs with its git
status (**A**dded / **M**odified / **D**eleted / **R**enamed). Diff bands show
where each ref diverges from your current file; the `»` / `«` gizmos pull a hunk
from either ref into the center, and you can type freely.

- **Save** writes the center pane to the working-tree file. Nothing is ever
  staged — review with `git diff` afterwards.
- **Close** exits without touching anything (it confirms first if you have
  unsaved edits).
- Binary files are listed but not openable as text.

The command is script-friendly for editor/IDE integration: it blocks until the
window closes, exits `0` on a normal close, and exits `2` with a usage message
on bad arguments (`mcr diff <refA> <refB> [dir]` — the optional `dir` anchors
the repository when the caller's working directory isn't inside it).

### Use as a `git mergetool`

MCR honors Git's mergetool contract: it reads `LOCAL` / `BASE` / `REMOTE`,
resolves into `MERGED`, and exits `0` on **Save & Exit** or non-zero on
**Abort**. When invoked with four file paths it opens those files instead of the
demo; otherwise it runs standalone.

**The installer registers everything for you** — it writes a foreground
`mcr-mergetool` shim to `~/.local/bin` and configures `mergetool.mcr.*`
globally (it only sets `merge.tool mcr` if you had no merge tool yet).

To configure manually, point the `.cmd` at the **shim** — it blocks until the
window closes and resolves Git's relative paths to absolute (required on Linux,
where the AppImage changes directory at startup):

```bash
git config --global merge.tool mcr
git config --global mergetool.mcr.cmd \
  '"$HOME/.local/bin/mcr-mergetool" "$LOCAL" "$BASE" "$REMOTE" "$MERGED"'
git config --global mergetool.mcr.trustExitCode true
```

**Per-project instead of global:** run the same commands with `--local` (the
default scope inside a repo) so only that project uses MCR — it writes to
`.git/config`.

Then, on a conflicted repo:

```bash
git merge some-branch     # produces conflicts
git mergetool             # opens MCR for each conflicted file
```

Resolve with the apply/revert gizmos, click **Save & Exit** (writes `MERGED`,
exit 0) to mark the file resolved, or **Abort** (exit 1) to leave it conflicted.
`trustExitCode true` tells Git to believe those codes.

---

## Develop

```bash
cd src-tauri
cargo tauri dev      # starts the Vite dev server and opens the window
```

Tests and checks:

```bash
cargo test -p mcr-core            # engine: diff3, alignment, apply/revert round-trip
npm --prefix ui run test          # frontend: panes, decorations, connectors
npm --prefix ui run typecheck     # TypeScript
bash scripts/vendor-neutral-check.sh   # branding lint
```

### Layout

```
crates/mcr-core/   Rust merge engine (all diff/merge/apply/revert logic)
src-tauri/         Tauri shell + IPC commands
ui/                TypeScript + CodeMirror frontend (renders state, no merge logic)
```

---

## Releases

Pushing a `v*` tag triggers `.github/workflows/release.yml`:

- `tauri-action` builds the bundles — `.dmg` (macOS arm64), `.AppImage` + `.deb`
  (Linux x86_64) — and publishes a GitHub Release.
- The `cask` job regenerates the Homebrew cask (`Casks/mcr.rb`) in the tap with
  the new dmg + sha256 (requires a `HOMEBREW_TAP_GITHUB_TOKEN` secret with push
  access to the tap; skipped if unset).

The bundle version is synced from the git tag at build time so asset filenames
match the URLs the installer and cask construct.

---

## Support

If MCR saves you time, you can support its development:

<a href="https://www.buymeacoffee.com/mayckol" target="_blank">
  <img src="https://img.shields.io/badge/Buy%20me%20a%20coffee-FFDD00?style=for-the-badge&logo=buymeacoffee&logoColor=black" alt="Buy me a coffee" />
</a>

☕ **[buymeacoffee.com/mayckol](https://www.buymeacoffee.com/mayckol)**

## Contact

Mayckol Ferreira — **[mayckol.dev](https://mayckol.dev)** (links & contacts).

## License

[MIT](LICENSE) © Mayckol Ferreira
