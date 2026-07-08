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

/// Which index stages exist for one unmerged path.
#[derive(Clone, Copy, Debug, Default)]
pub struct StageSet {
    pub base: bool,
    pub local: bool,
    pub incoming: bool,
}

/// Every unmerged path with its present stages, from ONE `git ls-files -u` over
/// the whole index — no per-file subprocesses and no worktree stat scan (unlike
/// `git diff`), so it stays fast in large repositories.
pub fn unmerged_stage_sets(root: &str) -> Result<Vec<(String, StageSet)>, String> {
    let out = git(root, &["ls-files", "-u", "-z"])?;
    let mut order: Vec<String> = Vec::new();
    let mut sets: std::collections::HashMap<String, StageSet> = std::collections::HashMap::new();
    // Each record: "<mode> <sha> <stage>\t<path>", NUL-terminated.
    for rec in out.split(|&b| b == 0).filter(|s| !s.is_empty()) {
        let rec = String::from_utf8_lossy(rec);
        let Some((meta, path)) = rec.split_once('\t') else { continue };
        let Some(stage) = meta.split_whitespace().nth(2).and_then(|s| s.parse::<u8>().ok()) else {
            continue;
        };
        let entry = sets.entry(path.to_string()).or_insert_with(|| {
            order.push(path.to_string());
            StageSet::default()
        });
        match stage {
            STAGE_BASE => entry.base = true,
            STAGE_LOCAL => entry.local = true,
            STAGE_INCOMING => entry.incoming = true,
            _ => {}
        }
    }
    Ok(order.into_iter().map(|p| { let s = sets[&p]; (p, s) }).collect())
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

/// Provisional conflict kind from the stages alone — no blob reads. Binary is
/// only discoverable from content, so it is detected at session materialization.
pub fn kind_from_stages(stages: StageSet) -> ConflictKind {
    if !stages.local || !stages.incoming {
        return ConflictKind::DeleteModify;
    }
    if !stages.base {
        return ConflictKind::BothAdded;
    }
    ConflictKind::Text
}

/// The three raw sides of one conflicted file, straight from the index stages.
/// A missing stage (e.g. a deleted side) yields empty bytes so diff3 still aligns.
pub struct RawSides {
    pub base: Vec<u8>,
    pub local: Vec<u8>,
    pub incoming: Vec<u8>,
}

pub fn reconstruct_raw_sides(root: &str, path: &str) -> RawSides {
    RawSides {
        base: stage_blob(root, STAGE_BASE, path),
        local: stage_blob(root, STAGE_LOCAL, path),
        incoming: stage_blob(root, STAGE_INCOMING, path),
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

/// Repository root for an explicit DIRECTORY anchor (`mcr diff <ref> [dir]`).
/// `repo_root` is for file paths and takes the parent first — routing a
/// directory through it anchored at the directory's parent (Rust normalizes a
/// trailing `.` away, so the old `join(".")` compensation never held) and
/// "not inside a git repository" followed.
pub fn repo_root_dir(dir: &str) -> Option<String> {
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
