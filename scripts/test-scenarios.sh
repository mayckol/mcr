#!/usr/bin/env bash
#
# test-scenarios.sh — build the current MCR sources and open the built binary on a
# throwaway repo whose merge reproduces every conflict scenario MCR handles. The
# text files are ~260 lines each so scroll/connector behaviour is exercised.
#
#   1. text_conflict.go — conflicting + added + removed + modified hunks   -> Text
#   2. text_addremove.go — same categories, different content              -> Text
#   3. both_added.go     — created on both branches, no base (diff view)   -> BothAdded
#   4. delete_modify.go  — deleted on ours, modified on theirs (empty side)-> DeleteModify
#   5. logo.bin          — NUL-bearing blob changed on both sides          -> Binary
#
# The fixture repo lives in a temp dir; on exit it is removed, so the template
# changes are fully reverted and nothing touches this repository's worktree.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$REPO_ROOT/src-tauri/target/debug/mcr-app"

WORK=""
cleanup() {
  if [[ -n "$WORK" && -d "$WORK" ]]; then
    rm -rf "$WORK"
    echo "reverted: removed fixture repo $WORK"
  fi
}
trap cleanup EXIT

# --- generator: a ~260-line Go file with variant-specific edits -----------------
# $1 variant: base|ours|theirs   $2 return-value offset (distinguishes files)
# Edit map (same line = a conflict when ours and theirs both change it):
#   apiVersion const  -> conflicting header (all three differ)
#   step120           -> conflicting body   (ours vs theirs)
#   step060           -> incoming-only modification (non-conflicting)
#   step200           -> removed on theirs            (non-conflicting)
#   stepLocalOnly     -> appended on ours             (non-conflicting)
gen_go() {
  local v="$1" off="$2" i
  printf 'package repo\n\n'
  case "$v" in
    ours)   printf 'const apiVersion = "v2-local"\n\n' ;;
    theirs) printf 'const apiVersion = "v2-incoming"\n\n' ;;
    *)      printf 'const apiVersion = "v1"\n\n' ;;
  esac
  for i in $(seq 1 250); do
    if [[ $i -eq 120 ]]; then
      case "$v" in
        ours)   printf 'func step%03d() int { return %d } // local\n'    "$i" $((i + off + 5000)) ;;
        theirs) printf 'func step%03d() int { return %d } // incoming\n' "$i" $((i + off + 9000)) ;;
        *)      printf 'func step%03d() int { return %d }\n'             "$i" $((i + off)) ;;
      esac
    elif [[ $i -eq 60 && "$v" == theirs ]]; then
      printf 'func step%03d() int { return %d } // incoming-only change\n' "$i" $((i + off + 1))
    elif [[ $i -eq 200 && "$v" == theirs ]]; then
      : # omitted on theirs -> removed hunk (non-conflicting)
    else
      printf 'func step%03d() int { return %d }\n' "$i" $((i + off))
    fi
  done
  [[ "$v" == ours ]] && printf '\nfunc stepLocalOnly() int { return 777 }\n'
  return 0
}

# --- 1. build the current sources ------------------------------------------------
# ui/dist is embedded into the binary at build time (beforeBuildCommand is empty),
# so the frontend must be built first. --debug --no-bundle produces a runnable
# binary without packaging (plain `cargo build` ships a blank webview).
echo "==> building ui"
npm --prefix "$REPO_ROOT/ui" run build

echo "==> building mcr binary (debug, no bundle)"
( cd "$REPO_ROOT/src-tauri" && cargo tauri build --debug --no-bundle )

[[ -x "$BIN" ]] || { echo "build did not produce $BIN" >&2; exit 1; }

# --- 2. base commit -------------------------------------------------------------
WORK="$(mktemp -d "${TMPDIR:-/tmp}/mcr-scenarios.XXXXXX")"
echo "==> fixture repo: $WORK"

git -C "$WORK" init -q
git -C "$WORK" config user.email test@mcr.local
git -C "$WORK" config user.name "MCR Test"
git -C "$WORK" config commit.gpgsign false

gen_go base 0   >"$WORK/text_conflict.go"
gen_go base 100 >"$WORK/text_addremove.go"
gen_go base 200 >"$WORK/delete_modify.go"
printf '\x00\x01MCR-LOGO-BASE\x02\x00' >"$WORK/logo.bin"

git -C "$WORK" add -A
git -C "$WORK" commit -qm "base"
# The default branch name varies (main/master); capture it for the merge back.
BASE_BRANCH="$(git -C "$WORK" branch --show-current)"

# --- 3. THEIRS (feature branch) -------------------------------------------------
git -C "$WORK" checkout -q -b feature
gen_go theirs 0   >"$WORK/text_conflict.go"
gen_go theirs 100 >"$WORK/text_addremove.go"
gen_go theirs 200 >"$WORK/delete_modify.go"          # theirs modifies it...
gen_go theirs 300 >"$WORK/both_added.go"             # ...and adds this file
printf '\x00\x01MCR-LOGO-INCOMING\x02\x00\x03' >"$WORK/logo.bin"
git -C "$WORK" add -A
git -C "$WORK" commit -qm "incoming changes"

# --- 4. OURS (base branch) ------------------------------------------------------
git -C "$WORK" checkout -q "$BASE_BRANCH"
gen_go ours 0   >"$WORK/text_conflict.go"
gen_go ours 100 >"$WORK/text_addremove.go"
git -C "$WORK" rm -q delete_modify.go                # ours deletes it (delete/modify)
gen_go ours 300 >"$WORK/both_added.go"               # both-added, different content
printf '\x00\x01MCR-LOGO-LOCAL\x02\x00\x04\x05' >"$WORK/logo.bin"
git -C "$WORK" add -A
git -C "$WORK" commit -qm "local changes"

# --- 5. trigger the merge and open the built binary -----------------------------
echo "==> merging feature into $BASE_BRANCH (expected to conflict)"
git -C "$WORK" merge --no-edit feature || true

echo "==> unmerged files:"
git -C "$WORK" diff --name-only --diff-filter=U | sed 's/^/    /'

# Git's mergetool contract is: mcr <LOCAL> <BASE> <REMOTE> <MERGED>. MCR only needs
# a path inside the worktree (from MERGED) to discover the full conflicted set from
# the index; pass a real conflicted file for all four positions.
MERGED="$WORK/text_conflict.go"
echo "==> launching mcr — resolve/close the window to finish"
"$BIN" "$MERGED" "$MERGED" "$MERGED" "$MERGED" || true

echo "==> mcr exited"
