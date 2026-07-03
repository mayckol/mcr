use crate::discovery::{self, ConflictKind};
use mcr_core::{MergeSession, Side, SessionModel, WhitespaceMode};
use std::collections::HashMap;
use std::sync::Mutex;

/// The four files Git's mergetool contract passes in.
#[derive(Clone, Debug)]
pub struct MergeFiles {
    pub local: String,
    pub base: String,
    pub remote: String,
    pub merged: String,
}

/// One conflicted file discovered in the merge, before its session is opened.
#[derive(Clone, Debug)]
pub struct DiscoveredFile {
    pub path_label: String,
    pub worktree_path: String,
}

/// How the app was launched. `passed` is the single file Git's current invocation
/// handed us (used for fallback and to identify the file Git is waiting on);
/// `files` is the full conflicted set discovered from `repo_root` (empty in demo
/// or single-file fallback).
#[derive(Default)]
pub struct Launch {
    pub passed: Option<MergeFiles>,
    pub repo_root: Option<String>,
    pub files: Vec<DiscoveredFile>,
    pub keep_backup: bool,
    /// Set when launched as `mcr diff <refA> <refB>` — compare mode.
    pub compare: Option<CompareSpec>,
}

/// A `mcr diff` invocation: the two refs and the files that differ between them.
pub struct CompareSpec {
    pub ref_a: String,
    pub ref_b: String,
    pub files: Vec<discovery::ChangedFile>,
}

/// Per-file metadata the multi-file session tracks alongside the live MergeSession.
#[derive(Clone, Debug)]
pub struct MergeFileEntry {
    pub path_label: String,
    pub worktree_path: String,
    pub kind: ConflictKind,
    pub resolved: bool,
    /// Resolved by `accept_special` writing raw blob bytes (or deleting the file).
    /// While set, the text session must never be written over that resolution.
    pub accepted_raw: bool,
    /// Compare mode only: git name-status letter (A/M/D/R/…) between the two refs.
    pub change_status: Option<String>,
}

/// One row of the file list handed to the UI (lazy — no full model).
#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub path_label: String,
    pub kind: ConflictKind,
    pub resolved: bool,
    pub remaining_conflicts: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change_status: Option<String>,
}

/// Derived progress over the whole conflicted set (FR-006).
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct SessionProgress {
    pub total: usize,
    pub resolved_count: usize,
    pub remaining_conflicts: usize,
    pub all_resolved: bool,
}

/// Outcome of a finish attempt (FR-007/FR-008/FR-017).
#[derive(Clone, Debug, serde::Serialize)]
pub struct FinishOutcome {
    pub all_resolved: bool,
    pub unresolved: Vec<String>,
}

struct RepoCtx {
    root: String,
    keep_backup: bool,
}

/// Framework-agnostic session store. Holds all open merge sessions and forwards
/// intents to `mcr-core`. Kept free of Tauri types so it is unit-testable.
#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, MergeSession>>,
    merged_paths: Mutex<HashMap<String, String>>,
    entries: Mutex<HashMap<String, MergeFileEntry>>,
    order: Mutex<Vec<String>>,
    repo: Mutex<Option<RepoCtx>>,
    /// Session id of the file Git's current invocation is blocked on (drives the
    /// exit code so an unresolved file is never reported resolved).
    git_passed: Mutex<Option<String>>,
    counter: Mutex<u64>,
}

fn canonical(path: &str) -> String {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

fn side_from_str(s: &str) -> Option<Side> {
    match s {
        "local" => Some(Side::Local),
        "incoming" => Some(Side::Incoming),
        _ => None,
    }
}

impl SessionManager {
    pub fn new() -> Self {
        Self::default()
    }

    fn next_id(&self) -> String {
        let mut c = self.counter.lock().unwrap();
        *c += 1;
        format!("session-{}", *c)
    }

    pub fn open(
        &self,
        local: &str,
        ancestor: &str,
        incoming: &str,
        whitespace_mode: Option<&str>,
    ) -> SessionModel {
        let id = self.next_id();
        let mode = WhitespaceMode::from_str_opt(whitespace_mode);
        let session = MergeSession::open(id.clone(), local, ancestor, incoming, mode);
        let model = session.to_model();
        self.sessions.lock().unwrap().insert(id, session);
        model
    }

    /// Open a session from Git's mergetool files and remember where to write the
    /// resolution. `base` may be empty (a 2-way merge with no common ancestor).
    pub fn open_files(&self, files: &MergeFiles) -> Result<SessionModel, String> {
        let local = std::fs::read_to_string(&files.local)
            .map_err(|e| format!("read LOCAL {}: {e}", files.local))?;
        let base = std::fs::read_to_string(&files.base).unwrap_or_default();
        let remote = std::fs::read_to_string(&files.remote)
            .map_err(|e| format!("read REMOTE {}: {e}", files.remote))?;
        let id = self.next_id();
        let session = MergeSession::open(id.clone(), &local, &base, &remote, WhitespaceMode::None);
        let model = session.to_model();
        self.sessions.lock().unwrap().insert(id.clone(), session);
        self.merged_paths.lock().unwrap().insert(id, files.merged.clone());
        Ok(model)
    }

    /// Write the current result to the session's MERGED path (mergetool contract).
    pub fn save_merged(&self, session_id: &str) -> Result<(), String> {
        let text = {
            let map = self.sessions.lock().unwrap();
            let session = map
                .get(session_id)
                .ok_or_else(|| format!("unknown session: {session_id}"))?;
            session.to_model().panes.result.join("\n")
        };
        let path = self
            .merged_paths
            .lock()
            .unwrap()
            .get(session_id)
            .cloned()
            .ok_or_else(|| format!("no MERGED path for session: {session_id}"))?;
        // Multi-file sides come from index blobs, which are repo-normalized (LF)
        // under core.autocrlf — but the checkout on disk may be CRLF. Match the
        // existing worktree file's endings so resolving never flips a file's EOLs.
        // Skip when the result already carries CRLF (single-file path uses Git's
        // smudged temp files, whose \r survives split/join).
        let text = match std::fs::read(&path) {
            Ok(existing)
                if existing.windows(2).any(|w| w == b"\r\n") && !text.contains("\r\n") =>
            {
                text.replace('\n', "\r\n")
            }
            _ => text,
        };
        std::fs::write(&path, text).map_err(|e| format!("write MERGED {path}: {e}"))?;
        Ok(())
    }

    /// Record the repository context so writes can stage and back up files.
    pub fn set_repo(&self, root: String, keep_backup: bool) {
        *self.repo.lock().unwrap() = Some(RepoCtx { root, keep_backup });
    }

    /// Open one discovered conflicted file as a session, recording its list entry
    /// and write-back path. Reconstructs the three sides from the index stages.
    pub fn open_entry(&self, root: &str, file: &DiscoveredFile) -> SessionModel {
        let kind = discovery::conflict_kind(root, &file.path_label);
        let sides = discovery::reconstruct_sides(root, &file.path_label);
        let id = self.next_id();
        let session = MergeSession::open(
            id.clone(),
            &sides.local,
            &sides.base,
            &sides.incoming,
            WhitespaceMode::None,
        );
        let model = session.to_model();
        let resolved = model.status.fully_resolved && kind == ConflictKind::Text;
        self.sessions.lock().unwrap().insert(id.clone(), session);
        self.merged_paths
            .lock()
            .unwrap()
            .insert(id.clone(), file.worktree_path.clone());
        self.entries.lock().unwrap().insert(
            id.clone(),
            MergeFileEntry {
                path_label: file.path_label.clone(),
                worktree_path: file.worktree_path.clone(),
                kind,
                resolved,
                accepted_raw: false,
                change_status: None,
            },
        );
        self.order.lock().unwrap().push(id);
        model
    }

    /// Open one compared file: local = blob at `ref_a`, incoming = blob at `ref_b`,
    /// base = the current worktree content — so the initial result projection IS
    /// the worktree file, and hunks show where each ref diverges from it. Binary
    /// (or non-UTF8) files are listed but get no session. Returns the model for
    /// text files.
    pub fn open_compare_entry(
        &self,
        root: &str,
        file: &discovery::ChangedFile,
        ref_a: &str,
        ref_b: &str,
    ) -> Option<SessionModel> {
        let path_at_a = file.old_path.as_deref().unwrap_or(&file.path);
        let local_bytes = discovery::ref_blob(root, ref_a, path_at_a);
        let incoming_bytes = discovery::ref_blob(root, ref_b, &file.path);
        let worktree_path = std::path::Path::new(root)
            .join(&file.path)
            .to_string_lossy()
            .into_owned();
        let base_bytes = std::fs::read(&worktree_path).unwrap_or_default();

        let label = match &file.old_path {
            Some(old) => format!("{old} → {}", file.path),
            None => file.path.clone(),
        };
        let id = self.next_id();
        let binary = discovery::blob_is_binary(&local_bytes)
            || discovery::blob_is_binary(&incoming_bytes)
            || discovery::blob_is_binary(&base_bytes);

        let (kind, model) = if binary {
            (ConflictKind::Binary, None)
        } else {
            let session = MergeSession::open(
                id.clone(),
                &String::from_utf8_lossy(&local_bytes),
                &String::from_utf8_lossy(&base_bytes),
                &String::from_utf8_lossy(&incoming_bytes),
                WhitespaceMode::None,
            );
            let model = session.to_model();
            self.sessions.lock().unwrap().insert(id.clone(), session);
            self.merged_paths
                .lock()
                .unwrap()
                .insert(id.clone(), worktree_path.clone());
            (ConflictKind::Text, Some(model))
        };
        self.entries.lock().unwrap().insert(
            id.clone(),
            MergeFileEntry {
                path_label: label,
                worktree_path,
                kind,
                resolved: false,
                accepted_raw: false,
                change_status: Some(file.status.clone()),
            },
        );
        self.order.lock().unwrap().push(id);
        model
    }

    fn entry_resolved(&self, session_id: &str) -> bool {
        // A text file is resolved when its session has no remaining conflicts; a
        // whole-file accept sets `resolved` directly (special kinds rely on that).
        let entry_resolved = self
            .entries
            .lock()
            .unwrap()
            .get(session_id)
            .map(|e| e.resolved)
            .unwrap_or(false);
        if entry_resolved {
            return true;
        }
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.to_model().status.fully_resolved)
            .unwrap_or(false)
    }

    fn summary(&self, session_id: &str) -> Option<SessionSummary> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(session_id)?;
        let remaining = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.to_model().status.remaining_conflicts)
            .unwrap_or(0);
        let resolved = entry.resolved || (entry.kind == ConflictKind::Text && remaining == 0);
        Some(SessionSummary {
            session_id: session_id.to_string(),
            path_label: entry.path_label.clone(),
            kind: entry.kind,
            resolved,
            remaining_conflicts: remaining,
            change_status: entry.change_status.clone(),
        })
    }

    /// The file list in stable discovery order (FR-012).
    pub fn summaries(&self) -> Vec<SessionSummary> {
        let order = self.order.lock().unwrap().clone();
        order.iter().filter_map(|id| self.summary(id)).collect()
    }

    /// Overall progress over every file entry (FR-006).
    pub fn progress(&self) -> SessionProgress {
        let summaries = self.summaries();
        let total = summaries.len();
        let resolved_count = summaries.iter().filter(|s| s.resolved).count();
        SessionProgress {
            total,
            resolved_count,
            remaining_conflicts: total - resolved_count,
            all_resolved: total > 0 && resolved_count == total,
        }
    }

    /// Directly mark which session Git is waiting on (single-file fallback path).
    pub fn set_git_passed_id(&self, id: String) {
        *self.git_passed.lock().unwrap() = Some(id);
    }

    /// Remember which open file Git's current invocation is waiting on.
    pub fn set_git_passed_by_worktree(&self, merged_path: &str) {
        let target = canonical(merged_path);
        let found = {
            let entries = self.entries.lock().unwrap();
            entries
                .iter()
                .find(|(_, e)| canonical(&e.worktree_path) == target)
                .map(|(id, _)| id.clone())
        };
        if let Some(id) = found {
            *self.git_passed.lock().unwrap() = Some(id);
        }
    }

    /// Exit code for Git: 0 only when the file Git passed is resolved (or unknown),
    /// so a non-zero code is a true "this file unresolved" signal (research R2).
    pub fn exit_code(&self) -> i32 {
        let id = self.git_passed.lock().unwrap().clone();
        match id {
            Some(id) if !self.entry_resolved(&id) => 1,
            _ => 0,
        }
    }

    /// Whether the file Git is waiting on is confirmed resolved. Used by the
    /// native window-close handler to default to abort when nothing is resolved
    /// (e.g. the frontend never ran), so unresolved content is never staged.
    pub fn git_passed_resolved(&self) -> bool {
        match self.git_passed.lock().unwrap().clone() {
            Some(id) => self.entry_resolved(&id),
            None => false,
        }
    }

    /// Read-only model for an already-open session (file selection).
    pub fn model(&self, session_id: &str) -> Result<SessionModel, String> {
        self.sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.to_model())
            .ok_or_else(|| format!("unknown session: {session_id}"))
    }

    fn backup_and_write(&self, session_id: &str) -> Result<(), String> {
        let (worktree_path, _label) = {
            let entries = self.entries.lock().unwrap();
            let e = entries
                .get(session_id)
                .ok_or_else(|| format!("unknown entry: {session_id}"))?;
            (e.worktree_path.clone(), e.path_label.clone())
        };
        let keep_backup = self
            .repo
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.keep_backup)
            .unwrap_or(false);
        if keep_backup {
            let orig = format!("{worktree_path}.orig");
            if !std::path::Path::new(&orig).exists() {
                let _ = std::fs::copy(&worktree_path, &orig);
            }
        }
        self.save_merged(session_id)
    }

    /// Write a resolved file's result and stage it (incremental persist, R3/R4).
    pub fn save_and_stage(&self, session_id: &str) -> Result<(), String> {
        // A raw accept (`accept_special`) already wrote blob bytes — or deleted the
        // file — and staged it; rewriting from the lossy text session would corrupt
        // a binary or resurrect the chosen deletion. Binary files are additionally
        // never writable from text, whatever their resolved state.
        let (kind, accepted_raw) = self
            .entries
            .lock()
            .unwrap()
            .get(session_id)
            .map(|e| (e.kind, e.accepted_raw))
            .ok_or_else(|| format!("unknown entry: {session_id}"))?;
        if accepted_raw || kind == ConflictKind::Binary {
            return Ok(());
        }
        self.backup_and_write(session_id)?;
        if let (Some(ctx), Some(label)) = (
            self.repo.lock().unwrap().as_ref(),
            self.entries
                .lock()
                .unwrap()
                .get(session_id)
                .map(|e| e.path_label.clone()),
        ) {
            discovery::stage_path(&ctx.root, &label)?;
        }
        if let Some(e) = self.entries.lock().unwrap().get_mut(session_id) {
            e.resolved = true;
        }
        Ok(())
    }

    /// Resolve a whole file to one side directly from the list (FR-009/FR-010).
    pub fn accept_file(&self, session_id: &str, from: &str) -> Result<SessionSummary, String> {
        let side = side_from_str(from).ok_or_else(|| format!("invalid side: {from}"))?;
        let kind = self
            .entries
            .lock()
            .unwrap()
            .get(session_id)
            .map(|e| e.kind)
            .ok_or_else(|| format!("unknown entry: {session_id}"))?;

        if kind == ConflictKind::Text {
            // Apply every non-conflicting region plus all conflicts from `side`.
            self.with(session_id, |s| {
                s.apply_non_conflicting(None);
                let model = s.to_model();
                let conflicts: Vec<usize> = model
                    .hunks
                    .iter()
                    .filter(|h| h.category == mcr_core::Category::Conflicting)
                    .map(|h| h.id)
                    .collect();
                let mut last = s.to_model();
                for id in conflicts {
                    last = s.apply(id, side);
                }
                last
            })?;
            self.save_and_stage(session_id)?;
        } else {
            self.accept_special(session_id, from)?;
        }
        self.summary(session_id)
            .ok_or_else(|| format!("unknown entry: {session_id}"))
    }

    /// Whole-file accept for non-text conflicts: write the chosen side's blob (or
    /// delete the file if that side removed it) and stage it.
    fn accept_special(&self, session_id: &str, from: &str) -> Result<(), String> {
        let (root, keep_backup) = {
            let repo = self.repo.lock().unwrap();
            let ctx = repo
                .as_ref()
                .ok_or_else(|| "no repository for special accept".to_string())?;
            (ctx.root.clone(), ctx.keep_backup)
        };
        let (worktree_path, label) = {
            let entries = self.entries.lock().unwrap();
            let e = entries
                .get(session_id)
                .ok_or_else(|| format!("unknown entry: {session_id}"))?;
            (e.worktree_path.clone(), e.path_label.clone())
        };
        if keep_backup {
            let orig = format!("{worktree_path}.orig");
            if !std::path::Path::new(&orig).exists() {
                let _ = std::fs::copy(&worktree_path, &orig);
            }
        }
        if discovery::side_exists(&root, from, &label) {
            let blob = discovery::side_blob(&root, from, &label);
            std::fs::write(&worktree_path, blob)
                .map_err(|e| format!("write {worktree_path}: {e}"))?;
        } else {
            let _ = std::fs::remove_file(&worktree_path);
        }
        discovery::stage_path(&root, &label)?;
        if let Some(e) = self.entries.lock().unwrap().get_mut(session_id) {
            e.resolved = true;
            e.accepted_raw = true;
        }
        Ok(())
    }

    /// The ordered ids of files still unresolved (for "next unresolved", FR-011).
    pub fn unresolved_order(&self) -> Vec<String> {
        let order = self.order.lock().unwrap().clone();
        order
            .into_iter()
            .filter(|id| !self.entry_resolved(id))
            .collect()
    }

    /// Save+stage every resolved file and report whether the whole merge is done.
    pub fn finish(&self) -> Result<FinishOutcome, String> {
        let order = self.order.lock().unwrap().clone();
        let mut unresolved = Vec::new();
        for id in &order {
            if self.entry_resolved(id) {
                self.save_and_stage(id)?;
            } else {
                let label = self
                    .entries
                    .lock()
                    .unwrap()
                    .get(id)
                    .map(|e| e.path_label.clone())
                    .unwrap_or_else(|| id.clone());
                unresolved.push(label);
            }
        }
        Ok(FinishOutcome {
            all_resolved: unresolved.is_empty() && !order.is_empty(),
            unresolved,
        })
    }

    fn with<F>(&self, session_id: &str, f: F) -> Result<SessionModel, String>
    where
        F: FnOnce(&mut MergeSession) -> SessionModel,
    {
        let mut map = self.sessions.lock().unwrap();
        let session = map
            .get_mut(session_id)
            .ok_or_else(|| format!("unknown session: {session_id}"))?;
        let model = f(session);
        drop(map);
        // Any editor mutation supersedes a prior accept: the text session is the
        // live resolution again, so saves must write it and resolved-ness must be
        // re-derived from the session's remaining conflicts.
        if let Some(e) = self.entries.lock().unwrap().get_mut(session_id) {
            e.accepted_raw = false;
            e.resolved = false;
        }
        Ok(model)
    }

    pub fn apply_change(&self, sid: &str, hunk_id: usize, from: &str) -> Result<SessionModel, String> {
        let side = side_from_str(from).ok_or_else(|| format!("invalid side: {from}"))?;
        self.with(sid, |s| s.apply(hunk_id, side))
    }

    pub fn apply_both(&self, sid: &str, hunk_id: usize, first: &str) -> Result<SessionModel, String> {
        let side = side_from_str(first).ok_or_else(|| format!("invalid side: {first}"))?;
        self.with(sid, |s| s.apply_both(hunk_id, side))
    }

    pub fn revert_change(&self, sid: &str, hunk_id: usize) -> Result<SessionModel, String> {
        self.with(sid, |s| s.revert(hunk_id))
    }

    pub fn apply_non_conflicting(&self, sid: &str, from: &str) -> Result<SessionModel, String> {
        let side = match from {
            "both" => None,
            other => Some(side_from_str(other).ok_or_else(|| format!("invalid side: {other}"))?),
        };
        self.with(sid, |s| s.apply_non_conflicting(side))
    }

    pub fn edit_result(
        &self,
        sid: &str,
        start: usize,
        end: usize,
        text: &str,
    ) -> Result<SessionModel, String> {
        self.with(sid, |s| s.edit_result(start, end, text))
    }

    pub fn set_full_result(&self, sid: &str, text: &str) -> Result<SessionModel, String> {
        self.with(sid, |s| s.set_full_result(text))
    }

    pub fn undo(&self, sid: &str) -> Result<SessionModel, String> {
        self.with(sid, |s| s.undo())
    }

    pub fn redo(&self, sid: &str) -> Result<SessionModel, String> {
        self.with(sid, |s| s.redo())
    }

    pub fn navigate(
        &self,
        sid: &str,
        direction: &str,
        from_hunk: Option<usize>,
    ) -> Result<Option<usize>, String> {
        let next = match direction {
            "next" => true,
            "prev" => false,
            other => return Err(format!("invalid direction: {other}")),
        };
        let map = self.sessions.lock().unwrap();
        let session = map
            .get(sid)
            .ok_or_else(|| format!("unknown session: {sid}"))?;
        Ok(session.navigate(next, from_hunk))
    }

    pub fn set_whitespace_mode(&self, sid: &str, mode: &str) -> Result<SessionModel, String> {
        let wm = WhitespaceMode::from_str_opt(Some(mode));
        self.with(sid, |s| s.set_whitespace_mode(wm))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_apply_undo_via_manager() {
        let m = SessionManager::new();
        let model = m.open("a-L\nb", "a\nb", "a\nb", None);
        let sid = model.session_id.clone();
        assert!(!model.hunks.is_empty());
        let id = model.hunks[0].id;

        let reverted = m.revert_change(&sid, id).unwrap();
        assert!(reverted.panes.result.contains(&"a".to_string()));

        let undone = m.undo(&sid).unwrap();
        assert!(undone.panes.result.contains(&"a-L".to_string()));
    }

    #[test]
    fn unknown_session_errors() {
        let m = SessionManager::new();
        assert!(m.undo("nope").is_err());
    }

    use std::path::Path;
    use std::process::Command;

    fn git_ok(dir: &Path, args: &[&str]) {
        let ok = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        assert!(ok || args[0] == "merge", "git {:?} failed", args);
    }

    fn setup_conflicted_repo(dir: &Path) {
        std::fs::create_dir_all(dir).unwrap();
        git_ok(dir, &["init", "-q"]);
        git_ok(dir, &["config", "user.email", "t@example.com"]);
        git_ok(dir, &["config", "user.name", "Test"]);
        git_ok(dir, &["config", "commit.gpgsign", "false"]);
        let write = |name: &str, body: &str| std::fs::write(dir.join(name), body).unwrap();
        write("a.txt", "base\n");
        write("b.txt", "base\n");
        git_ok(dir, &["add", "."]);
        git_ok(dir, &["commit", "-q", "-m", "base"]);
        let main = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        git_ok(dir, &["checkout", "-q", "-b", "feature"]);
        write("a.txt", "feature\n");
        write("b.txt", "feature\n");
        git_ok(dir, &["commit", "-q", "-am", "feature"]);
        git_ok(dir, &["checkout", "-q", &main]);
        write("a.txt", "main\n");
        write("b.txt", "main\n");
        git_ok(dir, &["commit", "-q", "-am", "main"]);
        // Conflicts in a.txt and b.txt; merge returns non-zero (allowed).
        git_ok(dir, &["merge", "feature"]);
    }

    #[test]
    fn multifile_discovery_accept_finish() {
        if Command::new("git").arg("--version").output().is_err() {
            return; // git not available in this environment
        }
        let dir = std::env::temp_dir().join(format!("mcr-mf-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        setup_conflicted_repo(&dir);

        let root = discovery::repo_root(&dir.join("a.txt").to_string_lossy()).expect("repo root");
        let mut unmerged = discovery::unmerged_paths(&root).unwrap();
        unmerged.sort();
        assert_eq!(unmerged, vec!["a.txt".to_string(), "b.txt".to_string()]);
        assert_eq!(discovery::conflict_kind(&root, "a.txt"), ConflictKind::Text);

        let m = SessionManager::new();
        m.set_repo(root.clone(), false);
        let files: Vec<DiscoveredFile> = unmerged
            .iter()
            .map(|rel| DiscoveredFile {
                path_label: rel.clone(),
                worktree_path: Path::new(&root).join(rel).to_string_lossy().into_owned(),
            })
            .collect();
        let ids: Vec<String> = files
            .iter()
            .map(|f| m.open_entry(&root, f).session_id)
            .collect();

        let p0 = m.progress();
        assert_eq!(p0.total, 2);
        assert_eq!(p0.resolved_count, 0);

        // Accept local (ours = main) on a.txt; finish still blocked by b.txt.
        let s = m.accept_file(&ids[0], "local").unwrap();
        assert!(s.resolved);
        let blocked = m.finish().unwrap();
        assert!(!blocked.all_resolved);
        assert_eq!(blocked.unresolved.len(), 1);

        // Accept incoming (theirs = feature) on b.txt; now fully resolved.
        m.accept_file(&ids[1], "incoming").unwrap();
        let done = m.finish().unwrap();
        assert!(done.all_resolved, "unresolved: {:?}", done.unresolved);

        let a = std::fs::read_to_string(Path::new(&root).join("a.txt")).unwrap();
        assert!(a.contains("main"), "a.txt = {a:?}");
        let b = std::fs::read_to_string(Path::new(&root).join("b.txt")).unwrap();
        assert!(b.contains("feature"), "b.txt = {b:?}");
        assert!(discovery::unmerged_paths(&root).unwrap().is_empty());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn mergetool_files_roundtrip() {
        let dir = std::env::temp_dir().join("mcr-test-merge");
        std::fs::create_dir_all(&dir).unwrap();
        let p = |n: &str| dir.join(n).to_string_lossy().to_string();
        std::fs::write(p("LOCAL"), "title: LOCAL\nshared\n").unwrap();
        std::fs::write(p("BASE"), "title: BASE\nshared\n").unwrap();
        std::fs::write(p("REMOTE"), "title: REMOTE\nshared\n").unwrap();
        std::fs::write(p("MERGED"), "<<< conflict markers >>>").unwrap();

        let m = SessionManager::new();
        let files = MergeFiles {
            local: p("LOCAL"),
            base: p("BASE"),
            remote: p("REMOTE"),
            merged: p("MERGED"),
        };
        let model = m.open_files(&files).unwrap();
        let conflict = model.hunks.iter().find(|h| h.category == mcr_core::Category::Conflicting).unwrap();

        // Resolve the conflict to the local side, then write MERGED.
        m.apply_change(&model.session_id, conflict.id, "local").unwrap();
        m.save_merged(&model.session_id).unwrap();

        let written = std::fs::read_to_string(p("MERGED")).unwrap();
        assert!(written.contains("title: LOCAL"));
        assert!(!written.contains("conflict markers"));
    }

    #[test]
    fn save_merged_matches_crlf_worktree_endings() {
        let dir = std::env::temp_dir().join("mcr-test-crlf");
        std::fs::create_dir_all(&dir).unwrap();
        let p = |n: &str| dir.join(n).to_string_lossy().to_string();
        // Sides are LF (as index blobs are under autocrlf) but the checkout on
        // disk is CRLF; saving must not flip the worktree file's endings.
        std::fs::write(p("LOCAL"), "title: LOCAL\nshared\n").unwrap();
        std::fs::write(p("BASE"), "title: BASE\nshared\n").unwrap();
        std::fs::write(p("REMOTE"), "title: REMOTE\nshared\n").unwrap();
        std::fs::write(p("MERGED"), "<<< markers >>>\r\nshared\r\n").unwrap();

        let m = SessionManager::new();
        let files = MergeFiles {
            local: p("LOCAL"),
            base: p("BASE"),
            remote: p("REMOTE"),
            merged: p("MERGED"),
        };
        let model = m.open_files(&files).unwrap();
        let conflict = model
            .hunks
            .iter()
            .find(|h| h.category == mcr_core::Category::Conflicting)
            .unwrap();
        m.apply_change(&model.session_id, conflict.id, "local")
            .unwrap();
        m.save_merged(&model.session_id).unwrap();

        let written = std::fs::read_to_string(p("MERGED")).unwrap();
        assert!(written.contains("title: LOCAL\r\n"), "written = {written:?}");
        assert!(!written.contains("shared\n\n"), "written = {written:?}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn raw_accept_blocks_text_rewrite_until_editor_mutation() {
        let dir = std::env::temp_dir().join("mcr-test-rawaccept");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("blob.bin").to_string_lossy().to_string();
        std::fs::write(&target, b"\x00raw-bytes").unwrap();

        let m = SessionManager::new();
        let model = m.open("local\n", "base\n", "incoming\n", None);
        let sid = model.session_id.clone();
        m.merged_paths
            .lock()
            .unwrap()
            .insert(sid.clone(), target.clone());
        m.entries.lock().unwrap().insert(
            sid.clone(),
            MergeFileEntry {
                path_label: "blob.bin".into(),
                worktree_path: target.clone(),
                kind: ConflictKind::DeleteModify,
                resolved: true,
                accepted_raw: true,
                change_status: None,
            },
        );
        m.order.lock().unwrap().push(sid.clone());

        // finish() must not overwrite the raw accept with the text session.
        m.finish().unwrap();
        assert_eq!(std::fs::read(&target).unwrap(), b"\x00raw-bytes");

        // An editor mutation supersedes the accept: the text session is live again.
        let hunk = m.model(&sid).unwrap().hunks[0].id;
        m.apply_change(&sid, hunk, "local").unwrap();
        let e = m.entries.lock().unwrap().get(&sid).cloned().unwrap();
        assert!(!e.accepted_raw);
        assert!(!e.resolved);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Two branches diverging on tracked files, for compare-mode tests.
    /// main: f.txt="main", added.txt absent, gone.txt="keep"
    /// feature: f.txt="feature", added.txt="new", gone.txt deleted
    fn setup_compare_repo(dir: &Path) -> (String, String) {
        std::fs::create_dir_all(dir).unwrap();
        git_ok(dir, &["init", "-q"]);
        git_ok(dir, &["config", "user.email", "t@example.com"]);
        git_ok(dir, &["config", "user.name", "Test"]);
        git_ok(dir, &["config", "commit.gpgsign", "false"]);
        let write = |name: &str, body: &str| std::fs::write(dir.join(name), body).unwrap();
        write("f.txt", "one\nmain\nthree\n");
        write("gone.txt", "keep\n");
        git_ok(dir, &["add", "."]);
        git_ok(dir, &["commit", "-q", "-m", "base"]);
        let main = String::from_utf8(
            Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .unwrap()
                .stdout,
        )
        .unwrap()
        .trim()
        .to_string();
        git_ok(dir, &["checkout", "-q", "-b", "feature"]);
        write("f.txt", "one\nfeature\nthree\n");
        write("added.txt", "new\n");
        git_ok(dir, &["add", "."]);
        git_ok(dir, &["rm", "-q", "gone.txt"]);
        git_ok(dir, &["commit", "-q", "-am", "feature"]);
        git_ok(dir, &["checkout", "-q", &main]);
        (main, "feature".to_string())
    }

    #[test]
    fn changed_paths_parses_statuses() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();

        let mut files = discovery::changed_paths(&root, &main, &feature).unwrap();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let statuses: Vec<(&str, &str)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(
            statuses,
            vec![("added.txt", "A"), ("f.txt", "M"), ("gone.txt", "D")]
        );

        // Rename detection carries the old path.
        git_ok(&dir, &["checkout", "-q", "-b", "renamer"]);
        git_ok(&dir, &["mv", "f.txt", "renamed.txt"]);
        git_ok(&dir, &["commit", "-q", "-am", "rename"]);
        let renamed = discovery::changed_paths(&root, &main, "renamer").unwrap();
        let r = renamed.iter().find(|f| f.status == "R").expect("R entry");
        assert_eq!(r.old_path.as_deref(), Some("f.txt"));
        assert_eq!(r.path, "renamed.txt");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_result_is_worktree_and_save_does_not_stage() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-save-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();
        // Dirty the worktree so it differs from both refs.
        std::fs::write(dir.join("f.txt"), "one\nworktree\nthree\n").unwrap();

        let m = SessionManager::new();
        let f = discovery::ChangedFile {
            status: "M".into(),
            path: "f.txt".into(),
            old_path: None,
        };
        let model = m.open_compare_entry(&root, &f, &main, &feature).unwrap();
        // The result pane starts as the CURRENT worktree content.
        assert_eq!(model.panes.result.join("\n"), "one\nworktree\nthree\n");
        assert_eq!(model.panes.local.join("\n"), "one\nmain\nthree\n");
        assert_eq!(model.panes.incoming.join("\n"), "one\nfeature\nthree\n");

        // Take the feature side for the diverging hunk and save.
        let hunk = model
            .hunks
            .iter()
            .find(|h| h.category == mcr_core::Category::Conflicting)
            .expect("both refs differ from worktree");
        m.apply_change(&model.session_id, hunk.id, "incoming").unwrap();
        m.save_merged(&model.session_id).unwrap();

        let written = std::fs::read_to_string(dir.join("f.txt")).unwrap();
        assert_eq!(written, "one\nfeature\nthree\n");
        // Nothing staged, no .orig backup.
        let staged = Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args(["diff", "--cached", "--name-only"])
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&staged.stdout).trim().is_empty());
        assert!(!dir.join("f.txt.orig").exists());

        let summary = m.summary(&model.session_id).unwrap();
        assert_eq!(summary.change_status.as_deref(), Some("M"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_added_and_deleted_sides_open_with_empty_panes() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-ad-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();

        let m = SessionManager::new();
        // added.txt exists only at `feature` (and not in the main-checkout worktree).
        let added = discovery::ChangedFile {
            status: "A".into(),
            path: "added.txt".into(),
            old_path: None,
        };
        let model = m.open_compare_entry(&root, &added, &main, &feature).unwrap();
        assert_eq!(model.panes.local.join("\n"), "");
        assert_eq!(model.panes.incoming.join("\n"), "new\n");

        // gone.txt exists at `main` but not at `feature`.
        let gone = discovery::ChangedFile {
            status: "D".into(),
            path: "gone.txt".into(),
            old_path: None,
        };
        let model = m.open_compare_entry(&root, &gone, &main, &feature).unwrap();
        assert_eq!(model.panes.local.join("\n"), "keep\n");
        assert_eq!(model.panes.incoming.join("\n"), "");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_binary_listed_without_session() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-bin-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();
        git_ok(&dir, &["checkout", "-q", "-b", "bin"]);
        std::fs::write(dir.join("blob.bin"), b"\x00\x01\x02").unwrap();
        git_ok(&dir, &["add", "."]);
        git_ok(&dir, &["commit", "-q", "-m", "bin"]);
        git_ok(&dir, &["checkout", "-q", &main]);
        let _ = feature;

        let m = SessionManager::new();
        let f = discovery::ChangedFile {
            status: "A".into(),
            path: "blob.bin".into(),
            old_path: None,
        };
        assert!(m.open_compare_entry(&root, &f, &main, "bin").is_none());
        let summaries = m.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, ConflictKind::Binary);
        assert_eq!(summaries[0].change_status.as_deref(), Some("A"));
        assert!(m.model(&summaries[0].session_id).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_summary_serializes_without_change_status() {
        let s = SessionSummary {
            session_id: "s".into(),
            path_label: "a.txt".into(),
            kind: ConflictKind::Text,
            resolved: false,
            remaining_conflicts: 1,
            change_status: None,
        };
        let json = serde_json::to_string(&s).unwrap();
        assert!(!json.contains("change_status"), "json = {json}");
    }

    #[test]
    fn non_utf8_sides_classify_as_binary() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-latin1-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        git_ok(&dir, &["init", "-q"]);
        git_ok(&dir, &["config", "user.email", "t@example.com"]);
        git_ok(&dir, &["config", "user.name", "Test"]);
        git_ok(&dir, &["config", "commit.gpgsign", "false"]);
        // Latin-1 "café" — no NUL bytes, but invalid UTF-8; a lossy text session
        // would corrupt it, so it must route through the raw-accept path.
        std::fs::write(dir.join("l1.txt"), b"caf\xe9 base\n").unwrap();
        git_ok(&dir, &["add", "."]);
        git_ok(&dir, &["commit", "-q", "-m", "base"]);
        git_ok(&dir, &["checkout", "-q", "-b", "feature"]);
        std::fs::write(dir.join("l1.txt"), b"caf\xe9 feature\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "feature"]);
        git_ok(&dir, &["checkout", "-q", "-"]);
        std::fs::write(dir.join("l1.txt"), b"caf\xe9 main\n").unwrap();
        git_ok(&dir, &["commit", "-q", "-am", "main"]);
        git_ok(&dir, &["merge", "feature"]);

        let root = discovery::repo_root(&dir.join("l1.txt").to_string_lossy()).unwrap();
        assert_eq!(discovery::conflict_kind(&root, "l1.txt"), ConflictKind::Binary);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
