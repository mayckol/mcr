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

/// A `git` Command that never flashes a console window: MCR is a GUI-subsystem
/// binary on Windows, where a plain child process allocates a visible console —
/// one flash per git call, several calls per file.
fn git_cmd() -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new("git");
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn git(root: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let out = git_cmd()
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
    let out = git_cmd()
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

// Non-UTF8 counts as binary: the text session is UTF-8 only, so a Latin-1/CP-1252
// file routed as text would be lossy-converted (every non-ASCII byte → U+FFFD) and
// the corruption written back on save. Binary kind resolves from raw blobs instead.
fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
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
    if is_binary(&stage_blob(root, STAGE_LOCAL, path)) || is_binary(&stage_blob(root, STAGE_INCOMING, path)) {
        return ConflictKind::Binary;
    }
    if !has_base {
        return ConflictKind::BothAdded;
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

/// One path changed between two refs (`git diff --name-status`).
#[derive(Clone, Debug)]
pub struct ChangedFile {
    /// First letter of the status: A, M, D, R, C, T.
    pub status: String,
    /// Path at the second ref (post-rename).
    pub path: String,
    /// Path at the first ref for renames/copies.
    pub old_path: Option<String>,
}

/// Repository root for the process working directory (compare-mode launch anchor).
pub fn repo_root_cwd() -> Option<String> {
    let out = git_cmd()
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

/// Whether `refspec` names a commit (branch, tag, or SHA) in the repository.
pub fn resolves_to_commit(root: &str, refspec: &str) -> bool {
    git_cmd()
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--verify", "--quiet", &format!("{refspec}^{{commit}}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Files that differ between a ref and the working tree (`git diff --name-status
/// -z <ref>`). Rename/copy records carry two paths (old, new); everything else
/// carries one. Status letters read ref → worktree: A = only in the worktree,
/// D = only at the ref.
pub fn changed_paths(root: &str, refspec: &str) -> Result<Vec<ChangedFile>, String> {
    let out = git(root, &["diff", "--name-status", "-z", refspec])?;
    let mut tokens = out
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned());
    let mut files = Vec::new();
    while let Some(status) = tokens.next() {
        let letter = status.chars().next().unwrap_or('M');
        let takes_two = matches!(letter, 'R' | 'C');
        let first = match tokens.next() {
            Some(p) => p,
            None => break,
        };
        let (old_path, path) = if takes_two {
            match tokens.next() {
                Some(new) => (Some(first), new),
                None => (None, first),
            }
        } else {
            (None, first)
        };
        files.push(ChangedFile {
            status: letter.to_string(),
            path,
            old_path,
        });
    }
    Ok(files)
}

/// Raw bytes of a path's blob at an arbitrary ref; empty when absent at that ref.
pub fn ref_blob(root: &str, refspec: &str, path: &str) -> Vec<u8> {
    git(root, &["show", &format!("{refspec}:{path}")]).unwrap_or_default()
}

/// Whether blob content is untextable (NUL bytes or invalid UTF-8).
pub fn blob_is_binary(bytes: &[u8]) -> bool {
    is_binary(bytes)
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
    git_cmd()
        .arg("-C")
        .arg(root)
        .args(["config", "--get", "mergetool.keepBackup"])
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() != "false")
        .unwrap_or(true)
}
