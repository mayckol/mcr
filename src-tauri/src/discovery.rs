//! Git plumbing for multi-file merges: find the repository, enumerate the unmerged
//! set, reconstruct each side from the index stages, and classify the conflict.
//!
//! Git's mergetool contract only ever hands MCR one file, so the full set is
//! discovered here from the repository the merge lives in (spec research R1/R5).

use std::path::Path;
use std::process::Command;

/// Stage numbers in Git's index for an unmerged path.
const STAGE_BASE: u8 = 1; // common ancestor
const STAGE_LOCAL: u8 = 2; // ours / LOCAL
const STAGE_INCOMING: u8 = 3; // theirs / REMOTE

/// What kind of conflict a path carries — drives the per-file UI (FR-002/FR-014).
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ConflictKind {
    Text,
    Binary,
    DeleteModify,
    BothAdded,
}

/// The three reconstructed sides of one conflicted file (text).
pub struct Sides {
    pub base: String,
    pub local: String,
    pub incoming: String,
}

fn git(root: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .map_err(|e| format!("git {}: {e}", args.join(" ")))?;
    if !out.status.success() {
        return Err(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    Ok(out.stdout)
}

/// Anchor the repository root from a path inside the worktree (the `$MERGED` file).
/// Returns `None` when not inside a Git worktree (standalone fallback, R5).
pub fn repo_root(from: &str) -> Option<String> {
    let dir = Path::new(from).parent().unwrap_or_else(|| Path::new("."));
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let root = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if root.is_empty() {
        None
    } else {
        Some(root)
    }
}

/// Repo-relative paths of every unmerged (conflicted) file (`--diff-filter=U`).
pub fn unmerged_paths(root: &str) -> Result<Vec<String>, String> {
    let out = git(root, &["diff", "--name-only", "--diff-filter=U", "-z"])?;
    Ok(out
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect())
}

fn stages_present(root: &str, path: &str) -> Result<Vec<u8>, String> {
    let out = git(root, &["ls-files", "-u", "-z", "--", path])?;
    let text = String::from_utf8_lossy(&out);
    let mut stages = Vec::new();
    // Each record: "<mode> <sha> <stage>\t<path>" separated by NUL.
    for rec in text.split('\0').filter(|s| !s.is_empty()) {
        if let Some(meta) = rec.split('\t').next() {
            if let Some(stage) = meta.split_whitespace().nth(2) {
                if let Ok(n) = stage.parse::<u8>() {
                    if !stages.contains(&n) {
                        stages.push(n);
                    }
                }
            }
        }
    }
    Ok(stages)
}

fn stage_blob(root: &str, stage: u8, path: &str) -> Vec<u8> {
    git(root, &["show", &format!(":{stage}:{path}")]).unwrap_or_default()
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

/// Classify the conflict at `path` so the UI can route text vs. accept-only files.
pub fn conflict_kind(root: &str, path: &str) -> ConflictKind {
    let stages = stages_present(root, path).unwrap_or_default();
    let has_base = stages.contains(&STAGE_BASE);
    let has_local = stages.contains(&STAGE_LOCAL);
    let has_incoming = stages.contains(&STAGE_INCOMING);

    if !has_local || !has_incoming {
        return ConflictKind::DeleteModify;
    }
    if !has_base {
        return ConflictKind::BothAdded;
    }
    if is_binary(&stage_blob(root, STAGE_LOCAL, path)) || is_binary(&stage_blob(root, STAGE_INCOMING, path)) {
        return ConflictKind::Binary;
    }
    ConflictKind::Text
}

/// Reconstruct the three text sides from the index stages. A missing stage (e.g. a
/// deleted side) yields an empty string so diff3 still aligns it.
pub fn reconstruct_sides(root: &str, path: &str) -> Sides {
    let read = |stage: u8| String::from_utf8_lossy(&stage_blob(root, stage, path)).into_owned();
    Sides {
        base: read(STAGE_BASE),
        local: read(STAGE_LOCAL),
        incoming: read(STAGE_INCOMING),
    }
}

/// Raw bytes of one side's blob — for accepting a whole binary/special file.
pub fn side_blob(root: &str, side: &str, path: &str) -> Vec<u8> {
    let stage = if side == "incoming" { STAGE_INCOMING } else { STAGE_LOCAL };
    stage_blob(root, stage, path)
}

/// Stage a resolved path back into Git's index (`git add`).
pub fn stage_path(root: &str, path: &str) -> Result<(), String> {
    git(root, &["add", "--", path]).map(|_| ())
}

/// Whether a stage exists for `side` ("local"/"incoming") — false means that side
/// deleted the file, so accepting it means deleting the worktree file.
pub fn side_exists(root: &str, side: &str, path: &str) -> bool {
    let stage = if side == "incoming" { STAGE_INCOMING } else { STAGE_LOCAL };
    stages_present(root, path).map(|s| s.contains(&stage)).unwrap_or(false)
}

/// `mergetool.keepBackup` (default true) — whether to write `<path>.orig` backups.
pub fn keep_backup(root: &str) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["config", "--get", "mergetool.keepBackup"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() != "false")
        .unwrap_or(true)
}
