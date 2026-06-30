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
      ;;
    Linux)
      rm -f "$PREFIX/bin/mcr" "$PREFIX/bin/mcr.AppImage" 2>/dev/null || true
      rm -f "$PREFIX/share/applications/mcr.desktop" 2>/dev/null || true
      rm -f "$PREFIX/share/icons/hicolor/256x256/apps/mcr.png" 2>/dev/null || true
      command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$PREFIX/share/applications" >/dev/null 2>&1 || true
      command -v gtk-update-icon-cache  >/dev/null 2>&1 && gtk-update-icon-cache -f "$PREFIX/share/icons/hicolor" >/dev/null 2>&1 || true
      ;;
  esac
  log "removed MCR."
  exit 0
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
    MNT="$(hdiutil attach -nobrowse -quiet "$TMP/app.dmg" | tail -1 | awk '{ $1=""; $2=""; sub(/^  */,""); print }')"
    [ -d "$MNT/MCR.app" ] || { hdiutil detach -quiet "$MNT" 2>/dev/null || true; fail "MCR.app not found in dmg"; }
    rm -rf /Applications/MCR.app 2>/dev/null || true
    cp -R "$MNT/MCR.app" /Applications/ || { hdiutil detach -quiet "$MNT"; fail "copy to /Applications failed (try sudo)"; }
    hdiutil detach -quiet "$MNT" || true
    xattr -dr com.apple.quarantine /Applications/MCR.app 2>/dev/null || true
    log "installed: /Applications/MCR.app"
    log "first launch: right-click → Open (unsigned build)"
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

    # Wrapper: resolve a relative path arg to absolute before detaching (the
    # AppImage chdir's into its mount, so a relative arg would resolve against
    # /tmp/.mount_* instead of the shell cwd). No arg → open with no project.
    cat > "$BIN.new" <<WRAP
#!/bin/sh
app="$APP"
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
    case ":$PATH:" in *":$BIN_DIR:"*) : ;; *) log "add $BIN_DIR to your PATH" ;; esac
    ;;
  *) fail "unsupported OS: $os_raw" ;;
esac

log "done"
