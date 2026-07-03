#!/bin/sh
# MCR installer — curl | sh friendly.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | sh
#   curl -fsSL .../install.sh | MCR_VERSION=v0.1.0 sh
#   curl -fsSL .../install.sh | sh -s -- --uninstall
#
# Env:
#   MCR_VERSION  tag to install (default: latest release)
#   MCR_PREFIX   Linux install prefix; AppImage goes to $PREFIX/bin (default: $HOME/.local)
#   MCR_REPO     override repo slug (default: mayckol/mcr)
#
# macOS: installs MCR.app to /Applications (Apple Silicon).
# Linux: installs the AppImage to $PREFIX/bin/mcr (x86_64) and registers a desktop
#        launcher + icon under $PREFIX/share so it shows in the application menu.

set -eu

REPO="${MCR_REPO:-mayckol/mcr}"
PREFIX="${MCR_PREFIX:-$HOME/.local}"
VERSION="${MCR_VERSION:-}"

ACTION=install
for a in "$@"; do
  case "$a" in
    --uninstall|--purge|--remove) ACTION=uninstall ;;
    -h|--help) printf 'MCR installer: pass --uninstall to remove.\n'; exit 0 ;;
    *) printf 'warning: ignoring unknown flag: %s\n' "$a" >&2 ;;
  esac
done

log()  { printf '==> %s\n' "$*" >&2; }
fail() { printf 'error: %s\n' "$*" >&2; exit 1; }

if command -v curl >/dev/null 2>&1; then DL='curl -fsSL'; else
  command -v wget >/dev/null 2>&1 || fail "need curl or wget"; DL='wget -qO-'; fi

os_raw="$(uname -s)"; arch_raw="$(uname -m)"

uninstall() {
  log "uninstalling MCR ($os_raw)"
  case "$os_raw" in
    Darwin)
      rm -rf "/Applications/MCR.app" 2>/dev/null || true
      rm -f "$PREFIX/bin/mcr" "$PREFIX/bin/mcr-mergetool" 2>/dev/null || true
      ;;
    Linux)
      rm -f "$PREFIX/bin/mcr" "$PREFIX/bin/mcr.AppImage" "$PREFIX/bin/mcr-mergetool" 2>/dev/null || true
      rm -f "$PREFIX/share/applications/mcr.desktop" 2>/dev/null || true
      rm -f "$PREFIX/share/icons/hicolor/256x256/apps/mcr.png" 2>/dev/null || true
      command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
      command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
      ;;
  esac
  # Drop the git mergetool registration (leave any non-mcr merge.tool intact).
  if command -v git >/dev/null 2>&1; then
    [ "$(git config --global --get merge.tool 2>/dev/null || true)" = mcr ] && git config --global --unset merge.tool 2>/dev/null || true
    git config --global --remove-section mergetool.mcr 2>/dev/null || true
  fi
  log "removed MCR."
  exit 0
}

# Register MCR as a git mergetool. Writes a FOREGROUND shim that hands git's four
# files to MCR with absolute paths (so the AppImage's chdir-into-mount can't
# misresolve a relative $MERGED), then points `git mergetool --tool=mcr` at it.
# $1 = the MCR binary to exec (AppImage on Linux, the .app's Mach-O on macOS).
install_mergetool() {
  app_bin="$1"
  mkdir -p "$PREFIX/bin"
  shim="$PREFIX/bin/mcr-mergetool"

  cat > "$shim.new" <<'SHIM'
#!/bin/sh
# MCR git-mergetool adapter. git invokes:
#   mcr-mergetool <LOCAL> <BASE> <REMOTE> <MERGED>
# It MUST run in the foreground and exit with MCR's status, so set
# `git config mergetool.mcr.trustExitCode true` (the installer does this).
set -eu
abspath() {
  case "$1" in
    /*) printf '%s\n' "$1" ;;
    *)  ( cd "$(dirname "$1")" 2>/dev/null && printf '%s/%s\n' "$(pwd)" "$(basename "$1")" ) \
          || printf '%s/%s\n' "$(pwd)" "$1" ;;
  esac
}
L="${1:-}"; B="${2:-}"; R="${3:-}"; M="${4:-}"
[ -n "$L" ] && L="$(abspath "$L")"
[ -n "$B" ] && B="$(abspath "$B")"
[ -n "$R" ] && R="$(abspath "$R")"
[ -n "$M" ] && M="$(abspath "$M")"
exec "@APP@" "$L" "$B" "$R" "$M"
SHIM
  # Substitute the real binary path (heredoc is quoted to avoid escaping the shim's
  # own $-expansions; @APP@ has no shell meaning so a plain sed swap is safe).
  sed "s#@APP@#${app_bin}#g" "$shim.new" > "$shim.sub" && mv -f "$shim.sub" "$shim.new"
  chmod +x "$shim.new"
  mv -f "$shim.new" "$shim"
  log "mergetool shim: $shim"

  if command -v git >/dev/null 2>&1; then
    git config --global mergetool.mcr.cmd "\"$shim\" \"\$LOCAL\" \"\$BASE\" \"\$REMOTE\" \"\$MERGED\"" || true
    git config --global mergetool.mcr.trustExitCode true || true
    # Don't clobber an existing global merge.tool; only default it when unset.
    if [ -z "$(git config --global --get merge.tool 2>/dev/null || true)" ]; then
      git config --global merge.tool mcr || true
      log "set global merge.tool=mcr — run: git mergetool"
    else
      log "kept merge.tool=$(git config --global --get merge.tool) — run: git mergetool --tool=mcr"
    fi
  else
    log "git not found — skipped mergetool registration (shim still installed)"
  fi
}

[ "$ACTION" = uninstall ] && uninstall

if [ -z "$VERSION" ]; then
  log "resolving latest release for $REPO"
  VERSION="$($DL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' | head -n1)"
  [ -n "$VERSION" ] || fail "could not resolve latest version"
fi
case "$VERSION" in v*) VER_NUM="${VERSION#v}" ;; *) VER_NUM="$VERSION"; VERSION="v$VERSION" ;; esac
BASE="https://github.com/$REPO/releases/download/$VERSION"

TMP="$(mktemp -d 2>/dev/null || mktemp -d -t mcr)"
trap 'rm -rf "$TMP"' EXIT INT HUP TERM

# Look up the real bundle filename for a tag instead of constructing it: Tauri
# stamps bundle names from the app's config version, which can lag the git tag,
# so a tag-built filename 404s whenever the two drift. Match by suffix regex.
resolve_asset() {
  $DL "https://api.github.com/repos/$REPO/releases/tags/$VERSION" 2>/dev/null \
    | sed -n 's/.*"name":[[:space:]]*"\(MCR[^"]*'"$1"'\)".*/\1/p' | head -n1
}

case "$os_raw" in
  Darwin)
    [ "$arch_raw" = "arm64" ] || fail "macOS build is Apple Silicon (arm64) only; got $arch_raw"
    ASSET="$(resolve_asset '_aarch64\.dmg')"; [ -n "$ASSET" ] || ASSET="MCR_${VER_NUM}_aarch64.dmg"
    log "downloading $ASSET"
    $DL "$BASE/$ASSET" > "$TMP/app.dmg" || fail "download failed: $BASE/$ASSET"
    log "mounting"
    # No -quiet: it silences the mount table on stdout, which is what we parse.
    MNT="$(hdiutil attach -nobrowse "$TMP/app.dmg" | tail -1 | awk '{ $1=""; $2=""; sub(/^  */,""); print }')"
    [ -n "$MNT" ] || fail "could not determine dmg mount point"
    [ -d "$MNT/MCR.app" ] || { hdiutil detach -quiet "$MNT" 2>/dev/null || true; fail "MCR.app not found in dmg"; }
    rm -rf /Applications/MCR.app 2>/dev/null || true
    cp -R "$MNT/MCR.app" /Applications/ || { hdiutil detach -quiet "$MNT"; fail "copy to /Applications failed (try sudo)"; }
    hdiutil detach -quiet "$MNT" || true
    xattr -dr com.apple.quarantine /Applications/MCR.app 2>/dev/null || true
    log "installed: /Applications/MCR.app"
    log "first launch: right-click → Open (unsigned build)"
    # The bundle's executable is named after the Cargo binary (mcr-app), not the
    # product name.
    install_mergetool "/Applications/MCR.app/Contents/MacOS/mcr-app"
    # `mcr` CLI shim: `mcr diff <refA> <refB>` etc. exec preserves the CWD, which
    # compare mode uses to find the repository.
    mkdir -p "$PREFIX/bin"
    cat > "$PREFIX/bin/mcr.new" <<'MCRCLI'
#!/bin/sh
exec "/Applications/MCR.app/Contents/MacOS/mcr-app" "$@"
MCRCLI
    chmod +x "$PREFIX/bin/mcr.new"
    mv -f "$PREFIX/bin/mcr.new" "$PREFIX/bin/mcr"
    log "cli: $PREFIX/bin/mcr (mcr diff <refA> <refB> to compare)"
    ;;
  Linux)
    case "$arch_raw" in x86_64|amd64) : ;; *) fail "Linux build is x86_64 only; got $arch_raw" ;; esac
    ASSET="$(resolve_asset '_amd64\.AppImage')"; [ -n "$ASSET" ] || ASSET="MCR_${VER_NUM}_amd64.AppImage"
    BIN_DIR="$PREFIX/bin"; mkdir -p "$BIN_DIR"
    APP="$BIN_DIR/mcr.AppImage"
    BIN="$BIN_DIR/mcr"
    log "downloading $ASSET"
    # Download to a sibling temp file and atomically rename over the old one so
    # updating works while the app is running (writing a busy executable in place
    # fails with "text file busy").
    $DL "$BASE/$ASSET" > "$APP.new" || { rm -f "$APP.new"; fail "download failed: $BASE/$ASSET"; }
    chmod +x "$APP.new"
    mv -f "$APP.new" "$APP"

    # Wrapper. `mcr diff <refA> <refB>` runs in the FOREGROUND (like git difftool)
    # and passes the caller's cwd as the repo anchor — the AppImage chdir's into
    # its /tmp/.mount_* mount, so cwd-based repo discovery would misresolve.
    # Other launches detach, resolving a relative path arg to absolute first.
    cat > "$BIN.new" <<WRAP
#!/bin/sh
app="$APP"
if [ "\${1:-}" = "diff" ]; then
  # Append the cwd anchor only when the caller didn't pass a dir themselves.
  if [ "\$#" -eq 2 ]; then exec "\$app" "\$@" "\$(pwd)"; fi
  exec "\$app" "\$@"
fi
if [ "\$#" -eq 0 ]; then
  nohup "\$app" >/dev/null 2>&1 &
else
  d="\$1"
  case "\$d" in
    /*) p="\$d" ;;
    *) p="\$(cd "\$d" 2>/dev/null && pwd)" || p="\$(pwd)/\$d" ;;
  esac
  nohup "\$app" "\$p" >/dev/null 2>&1 &
fi
WRAP
    chmod +x "$BIN.new"
    mv -f "$BIN.new" "$BIN"
    log "installed: $APP (launcher: $BIN)"

    APPS_DIR="$PREFIX/share/applications"
    ICON_DIR="$PREFIX/share/icons/hicolor/256x256/apps"
    mkdir -p "$APPS_DIR" "$ICON_DIR"
    $DL "https://raw.githubusercontent.com/$REPO/main/src-tauri/icons/128x128@2x.png" > "$ICON_DIR/mcr.png" 2>/dev/null || \
      log "could not install an icon (menu entry will use a generic one)"

    cat > "$APPS_DIR/mcr.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=MCR
Comment=Three-pane visual git merge editor
Exec="$APP"
Icon=mcr
Terminal=false
Categories=Development;Utility;
StartupWMClass=MCR
EOF
    chmod 644 "$APPS_DIR/mcr.desktop"

    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS_DIR" >/dev/null 2>&1 || true
    command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true

    log "menu entry: $APPS_DIR/mcr.desktop"
    install_mergetool "$APP"
    case ":$PATH:" in *":$BIN_DIR:"*) : ;; *) log "add $BIN_DIR to your PATH" ;; esac
    ;;
  *) fail "unsupported OS: $os_raw" ;;
esac

log "done"
