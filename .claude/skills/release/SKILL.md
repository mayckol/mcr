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

3.2 **Run the CI gates locally — BEFORE tagging.** `ci.yml` runs on the `main`
   push and gates with fmt/clippy/test; a tag that fails these wastes a build and
   leaves `main` red. Mirror every gate locally first and only proceed when all pass:
   ```sh
   cargo test -p mcr-core --all-features
   cargo fmt -p mcr-core -- --check        # if this fails: `cargo fmt -p mcr-core` then re-stage
   cargo clippy -p mcr-core -- -D warnings  # fix every warning; -D makes warnings fatal
   npm --prefix ui run typecheck
   npm --prefix ui test
   bash scripts/vendor-neutral-check.sh
   ```
   `cargo fmt -- --check` is the one most often missed: rustfmt reformats new
   multi-line `enum`/`assert!` code, and CI fails on the diff. Run `cargo fmt` and
   fold the result into the release commit. Note: CI clippy does NOT pass
   `--all-targets`, so it lints the lib only — don't be alarmed by dead-code
   warnings in test fixtures from a local `--all-targets` run.

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
   All jobs (builds + `cask`) should be green. If the build fails, report the
   failing job/log and do not proceed to verify. If only `cask` fails, see step 7.

6. **Verify assets match the tag.** The whole point — catch drift before users do:
   ```sh
   gh release view vX.Y.Z --repo mayckol/mcr --json assets --jq '.assets[].name'
   ```
   Confirm these exist with the **exact** version in the name:
   - `MCR_X.Y.Z_aarch64.dmg`
   - `MCR_X.Y.Z_amd64.AppImage`
   - `MCR_X.Y.Z_amd64.deb`
   - `MCR-X.Y.Z-1.x86_64.rpm`
   If any name shows a different version than `X.Y.Z`, the sync step failed —
   stop and fix before announcing.

7. **Verify the Homebrew cask updated.** As of v0.3.3 the `cask` CI job pushes the
   tap automatically (the `HOMEBREW_TAP_GITHUB_TOKEN` secret was fixed — see Notes).
   Just confirm the tap reflects the new version:
   ```sh
   gh api repos/mayckol/homebrew-tap/contents/Casks/mcr.rb --jq '.content' | base64 -d | grep -E 'version|sha256'
   ```
   The `version` must equal `X.Y.Z`.

   **If the cask job fails again** (403 / bad or wrong-scoped token), push manually
   with your own gh auth (you own the tap):

   ```sh
   # a. download the release dmg and sha256 it
   gh release download vX.Y.Z --repo mayckol/mcr --pattern "MCR_X.Y.Z_aarch64.dmg" --clobber
   shasum -a 256 MCR_X.Y.Z_aarch64.dmg

   # b. render Casks/mcr.rb (use python/heredoc, NOT sed into a placeholder —
   #    a sed slip once published sha256 "PLACEHOLDER"). Template mirrors the
   #    cask step in release.yml: version, sha256, url with MCR_<ver>_aarch64.dmg.

   # c. PUT it, passing the current file's blob sha so the update is accepted
   SHA=$(gh api repos/mayckol/homebrew-tap/contents/Casks/mcr.rb --jq .sha)
   gh api --method PUT repos/mayckol/homebrew-tap/contents/Casks/mcr.rb \
     -f message="chore: update mcr cask to vX.Y.Z" \
     -f content="$(base64 -i mcr.rb)" \
     -f sha="$SHA"
   ```
   Re-verify with the step-7 grep; `sha256` must match step a's digest.

   Prefer fixing the token over doing this by hand: a fine-grained PAT needs
   **Contents: Read-write** AND its **Repository access must include
   `mayckol/homebrew-tap`** (the v0.3.3 outage was a token scoped to `mayckol/mcr`
   only — writes to the tap 403'd while reads passed). Do NOT trust
   `gh api repos/mayckol/homebrew-tap --jq .permissions` to validate it — that
   field reports your user role (`push:true`) regardless of the token's repo
   scope. The only real check is a write: rerun the cask job (`gh run rerun <id>
   --failed` — GitHub re-runs FAILED jobs only, never successful ones).

8. **Smoke-test the installer (optional, recommended):**
   ```sh
   curl -fsSL https://raw.githubusercontent.com/mayckol/mcr/main/scripts/install.sh | MCR_VERSION=vX.Y.Z sh
   ```

## Notes

- The `cask` CI job needs a `HOMEBREW_TAP_GITHUB_TOKEN` secret — a fine-grained
  PAT with **Contents: Read-write** whose **Repository access includes
  `mayckol/homebrew-tap`**. Fixed 2026-07-03 (v0.3.3): the prior token had
  Contents:write but was scoped to `mayckol/mcr` only, so tap writes 403'd. The
  job now pushes the cask automatically. If it regresses, see step 7 for the
  token fix and the manual-push fallback.
- macOS builds are Apple Silicon only; Linux builds are x86_64 only.
- There is no `mcr` CLI binary yet, so the release ships GUI bundles + a macOS
  cask only. A cross-platform `brew install mayckol/tap/mcr` formula can be added
  once a CLI/mergetool binary exists.
- To re-release a botched tag: delete it remote + local, delete the GitHub
  release, fix config, re-tag. `gh release delete vX.Y.Z --cleanup-tag`.
