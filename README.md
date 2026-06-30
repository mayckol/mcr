# MCR

A three-pane visual git merge editor — trace changes across local / result /
incoming, then apply or revert each change. Built with a Rust core and a Tauri +
CodeMirror UI.

## Install

### macOS (Apple Silicon)

Homebrew:

```bash
brew install --cask mayckol/tap/mcr
```

Or curl:

```bash
curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh
```

Installs `MCR.app` to `/Applications`. The build is unsigned — on first launch,
right-click the app → **Open**.

### Linux (x86_64)

```bash
curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh
```

Installs the AppImage to `~/.local/bin/mcr` (a `mcr` launcher + an app-menu
entry). Pass `MCR_PREFIX=/usr/local` to change the prefix.

### Pin a version / uninstall

```bash
curl -fsSL .../install.sh | MCR_VERSION=v0.1.0 sh   # specific release
curl -fsSL .../install.sh | sh -s -- --uninstall     # remove
```

## Releases

Tagging `v*` triggers `.github/workflows/release.yml`:

- `tauri-action` builds the bundles — `.dmg` (macOS arm64), `.AppImage` + `.deb`
  (Linux x86_64) — and publishes them to a GitHub Release.
- The `cask` job regenerates the Homebrew cask (`Casks/mcr.rb`) in the tap with
  the new dmg + sha256. Requires a `HOMEBREW_TAP_GITHUB_TOKEN` secret with push
  access to the tap repo (skipped if unset).

The bundle version is synced from the git tag at build time so asset filenames
match the URLs the installer and cask construct.

## Develop

```bash
cd src-tauri
cargo tauri dev      # starts the Vite dev server and opens the window
```

- `cargo test -p mcr-core` — engine tests (diff3, alignment, apply/revert round-trip).
- `npm --prefix ui test` — frontend tests.
- `bash scripts/vendor-neutral-check.sh` — branding lint.
