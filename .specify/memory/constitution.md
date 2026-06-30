<!--
SYNC IMPACT REPORT
Version change: 1.0.0 → 1.1.0
Bump rationale: Added Technology Stack section mandating Rust core + Tauri/webview UI
(new normative section = MINOR).

Modified principles: none (titles unchanged since 1.0.0)
  - I. Clean-Room Original Implementation
  - II. Three-Way Visual Merge Experience
  - III. Drop-In Git Merge Editor
  - IV. Reversible, Non-Destructive Operations
  - V. First-Class Branch & Commit Comparison
  - VI. Vendor-Neutral Branding

Added sections:
  - Technology Stack (new in 1.1.0)
  - Compatibility & Integration Constraints (since 1.0.0)
  - Development Workflow & Quality Gates (since 1.0.0)

Removed sections: none

Templates requiring updates:
  - .specify/templates/plan-template.md ✅ aligned (generic "[Gates determined based on constitution file]")
  - .specify/templates/spec-template.md ✅ aligned (no constitution references)
  - .specify/templates/tasks-template.md ✅ aligned (no constitution references)

Follow-up TODOs: none — all placeholders resolved.
-->

# MCR Constitution

MCR is a Git merge, compare, apply, and revert tool that doubles as a drop-in merge
editor for any terminal.

## Core Principles

### I. Clean-Room Original Implementation
MCR MUST be built as original code. Forking, vendoring, copying, or statically/dynamically
linking an existing merge-editor or diff-tool codebase is prohibited. Only the *interaction
idea* of a polished IDE-style merge editor may be reproduced — never its source, assets, or
marks. Any third-party dependency MUST be a general-purpose library (Git plumbing, UI
toolkit, diff algorithm) and never a repackaged merge editor.
Rationale: The product's legal and architectural foundation depends on owning the
implementation, not inheriting one.

### II. Three-Way Visual Merge Experience
Conflict resolution MUST present the three-way model: the two diverging sides plus their
common ancestor, resolving into an editable result pane. Each conflict hunk MUST be
individually acceptable, rejectable, and editable, with the result reflecting choices live.
The UX adopts the familiar IDE-style merge interaction; the originating product MUST NOT be
named or implied.
Rationale: Three-way visual merging is the core value and the lowest-friction model for
resolving conflicts correctly.

### III. Drop-In Git Merge Editor
MCR MUST function as a default Git merge tool/editor invokable from any terminal. It MUST
honor Git's mergetool contract: accept the `LOCAL`, `REMOTE`, `BASE`, and `MERGED`
arguments, write the resolution to `MERGED`, and return exit codes Git interprets correctly
(0 = resolved, non-zero = aborted). Configuration as `merge.tool` / `mergetool.<name>.cmd`
(and editor invocation paths) MUST be documented and testable from a clean environment.
Rationale: Adoption requires that existing Git workflows call MCR with zero scripting glue.

### IV. Reversible, Non-Destructive Operations
Every operation that applies a change MUST be revertible. Comparison and inspection MUST be
read-only and MUST NOT mutate the working tree, index, or history. Destructive actions MUST
require explicit confirmation and MUST be backed by a recorded undo path (e.g. backup of
pre-change state) so apply and revert are symmetric.
Rationale: Trust in a merge tool collapses the first time it silently loses work.

### V. First-Class Branch & Commit Comparison
Comparing arbitrary branches and commits MUST be a primary, standalone capability — not a
side effect of merging. Comparison MUST surface per-file and per-hunk differences and allow
selectively applying or reverting individual changes between the compared references.
Rationale: Compare/apply/revert is a distinct workflow users reach for outside of conflicts.

### VI. Vendor-Neutral Branding
No user-facing text, documentation, code comment, identifier, asset, or marketing material
may name, depict, or imply the IDE whose interaction model inspired MCR. Borrowing is limited
to the *idea* of the experience. Reviews MUST reject any reference to that vendor.
Rationale: Protects MCR's independent identity and avoids trademark and attribution risk.

## Technology Stack

The core MUST be implemented in **Rust**. Rust is selected for native single-binary
distribution, no garbage-collector pauses (critical because `git mergetool` cold-starts the
process per conflict), memory safety that reinforces Principle I (no undefined behavior in
original code), and a mature ecosystem for the workload — diff engines (`imara-diff` /
`similar`) and Git plumbing (`gix` / `git2`). These choices are binding:

- The merge/diff/apply/revert engine and the Git mergetool entrypoint MUST be Rust.
- The desktop UI MUST use a native-webview shell (**Tauri**), not an embedded full browser
  runtime, to keep the binary small and cold-start fast; Electron and equivalents are
  prohibited for this reason.
- The UI frontend MAY be TypeScript with a code-editor component (e.g. CodeMirror) to render
  the editable three-pane merge; it MUST hold no merge logic — all resolution, apply, and
  revert logic lives in the Rust core.
- Cross-compilation to Linux, macOS, and Windows from the single Rust workspace MUST be
  maintained (Principle/Section: Compatibility).
- Swapping the core language or replacing the UI shell with a heavier runtime is a
  backward-incompatible change requiring a MAJOR version bump and explicit justification.

Rationale: Startup latency and large-file diff throughput are the two performance pressures
unique to a per-invocation merge editor; Rust + a lightweight webview shell minimizes both
while keeping one portable artifact.

## Compatibility & Integration Constraints

- MCR MUST integrate via Git's standard mergetool/difftool interfaces; no wrapper script
  beyond documented configuration may be required.
- MCR MUST operate from a terminal invocation and degrade clearly when launched outside a Git
  repository or with missing merge arguments.
- Cross-platform behavior (Linux, macOS, Windows) MUST be preserved; path, line-ending, and
  encoding handling MUST NOT corrupt the `MERGED` output.
- Exit-code and `MERGED`-file semantics are a contract: changes to them are breaking and
  trigger a MAJOR version bump.

## Development Workflow & Quality Gates

- Every PR MUST verify compliance with all principles above; reviewers MUST block merges that
  fork an existing tool (I), break the Git mergetool contract (III), introduce a
  non-revertible operation (IV), or reference the inspiration vendor (VI).
- Conflict-resolution and apply/revert logic MUST have automated tests, including a
  round-trip test proving apply→revert restores the original state.
- The mergetool integration MUST be validated end-to-end via a real `git mergetool`
  invocation in CI on each supported platform.

## Governance

This constitution supersedes other practices for the MCR project. Amendments MUST be
documented in this file, version-bumped per the policy below, and accompanied by updates to
any dependent templates. Versioning follows semantic rules: MAJOR for backward-incompatible
governance or principle removal/redefinition (including the mergetool exit-code/`MERGED`
contract), MINOR for added or materially expanded principles/sections, PATCH for
clarifications. All PRs and reviews MUST verify compliance; unavoidable complexity MUST be
justified in the PR description.

**Version**: 1.1.0 | **Ratified**: 2026-06-30 | **Last Amended**: 2026-06-30
