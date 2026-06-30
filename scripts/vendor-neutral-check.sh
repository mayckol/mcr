#!/usr/bin/env bash
# FR-017 / Constitution Principle VI: no user-facing code may name or imply the
# IDE whose interaction model inspired MCR. Scans shipped code only (not the spec,
# which quotes the original user request).
set -euo pipefail

ROOTS=("crates" "src-tauri/src" "ui/src" "ui/index.html")
PATTERN='jetbrains|intellij|goland|webstorm|pycharm|rider|fleet'

hits=0
for root in "${ROOTS[@]}"; do
  [ -e "$root" ] || continue
  if grep -RniE "$PATTERN" "$root" 2>/dev/null; then
    hits=1
  fi
done

if [ "$hits" -ne 0 ]; then
  echo "vendor-neutral check FAILED: forbidden vendor reference found in shipped code" >&2
  exit 1
fi
echo "vendor-neutral check passed: no forbidden vendor references in shipped code"
