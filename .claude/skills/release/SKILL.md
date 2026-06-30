---
name: release
description: Cut a new MCR release — bump version, tag, push, watch CI, verify published assets match the tag. Use when the user says "cut a release", "ship a version", "release mcr", "/release", or asks to publish a new build.
---

# MCR release

Tag-driven release. Pushing a `v*` tag triggers `.github/workflows/release.yml`,
which builds the Tauri bundles (macOS dmg, Linux AppImage + deb) and updates the
Homebrew cask in the tap.

## Critical invariant

The git tag and the app version in `src-tauri/tauri.conf.json` + `src-tauri/Cargo.toml`
**must match**. Tauri stamps bundle filenames from the config version
(`MCR_<ver>_amd64.AppImage`, `MCR_<ver>_aarch64.dmg`). If config lags the tag,
the published asset name won't match what `scripts/install.sh` and the Homebrew
cask construct → `curl | sh` and `brew install` 404.

Two safety nets already exist, but keep them honest:
- CI `Sync version from tag` step rewrites config from the tag before building.
- `install.sh` resolves the real asset name from the GitHub API (suffix match)
  before falling back to a constructed name.

This skill bumps config **in the commit** so the repo also reflects reality —
never rely on CI sync alone, or `main` drifts from its tags.

## Steps

0. If there are uncommitted changes, commit them first.

1. **Preflight.** Run:
   ```sh
   git rev-parse --abbrev-ref HEAD   # must be main
   git status --porcelain            # must be empty
   git fetch origin && git status -sb # must be up to date with origin/main
   gh release list --repo mayckol/mcr -L 3
   ```
   Abort if not on `main`, tree dirty, or behind origin. Note the latest tag.

2. **Pick the next version.** Read current from `src-tauri/tauri.conf.json`.
   Ask the user patch / minor / major if not specified. Compute `X.Y.Z` (no `v`).
   Confirm it's greater than the latest tag.

3. **Bump config** (no `v` prefix in files):
   - `src-tauri/tauri.conf.json` → `"version": "X.Y.Z"`
   - `src-tauri/Cargo.toml` → `version = "X.Y.Z"` (the `[package]` line)
   - Refresh the lock: `cd src-tauri && cargo update -p mcr-app --precise X.Y.Z 2>/dev/null || cargo check --offline 2>/dev/null; cd -`
     (Cargo.lock version line for `mcr-app` must match; a plain `cargo check`
     also rewrites it.)

3.1 **Update the changelog** (`CHANGELOG.md`): add a `## vX.Y.Z` section.

4. **Commit + tag + push:**
   ```sh
   git add src-tauri/tauri.conf.json src-tauri/Cargo.toml src-tauri/Cargo.lock CHANGELOG.md
   git commit -m "chore(release): vX.Y.Z"
   git tag vX.Y.Z
   git push origin main
   git push origin vX.Y.Z
   ```
   Push `main` before the tag so CI checks out a commit that already carries the
   matching version.

5. **Watch CI:**
   ```sh
   gh run watch --repo mayckol/mcr $(gh run list --repo mayckol/mcr --workflow release.yml -L1 --json databaseId --jq '.[0].databaseId') --exit-status
   ```
   If the build fails, report the failing job/log; do not proceed to verify.

6. **Verify assets match the tag.** The whole point — catch drift before users do:
   ```sh
   gh release view vX.Y.Z --repo mayckol/mcr --json assets --jq '.assets[].name'
   ```
   Confirm these exist with the **exact** version in the name:
   - `MCR_X.Y.Z_aarch64.dmg`
   - `MCR_X.Y.Z_amd64.AppImage`
   - `MCR_X.Y.Z_amd64.deb`
   If any name shows a different version than `X.Y.Z`, the sync step failed —
   stop and fix before announcing.

7. **Verify the Homebrew cask updated** (only if the tap token is configured):
   ```sh
   gh api repos/mayckol/homebrew-tap/contents/Casks/mcr.rb --jq '.content' | base64 -d | grep -E 'version|sha256'
   ```
   The `version` must equal `X.Y.Z`. If `mcr.rb` is missing, the cask job skipped
   (no `HOMEBREW_TAP_GITHUB_TOKEN`) — set the secret and re-run the `cask` job.

8. **Smoke-test the installer (optional, recommended):**
   ```sh
   curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | MCR_VERSION=vX.Y.Z sh
   ```

## Notes

- The `cask` CI job needs a `HOMEBREW_TAP_GITHUB_TOKEN` secret (a PAT with push
  access to `mayckol/homebrew-tap`); it self-skips if unset. Until it runs once,
  `brew install --cask mayckol/tap/mcr` fails with "Cask 'mcr' is unavailable"
  because `Casks/mcr.rb` does not exist yet.
- macOS builds are Apple Silicon only; Linux builds are x86_64 only.
- There is no `mcr` CLI binary yet, so the release ships GUI bundles + a macOS
  cask only. A cross-platform `brew install mayckol/tap/mcr` formula can be added
  once a CLI/mergetool binary exists.
- To re-release a botched tag: delete it remote + local, delete the GitHub
  release, fix config, re-tag. `gh release delete vX.Y.Z --cleanup-tag`.
