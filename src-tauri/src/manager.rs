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
}

/// Per-file metadata the multi-file session tracks alongside the live MergeSession.
#[derive(Clone, Debug)]
pub struct MergeFileEntry {
    pub path_label: String,
    pub worktree_path: String,
    pub kind: ConflictKind,
    pub resolved: bool,
}

/// One row of the file list handed to the UI (lazy — no full model).
#[derive(Clone, Debug, serde::Serialize)]
pub struct SessionSummary {
    pub session_id: String,
    pub path_label: String,
    pub kind: ConflictKind,
    pub resolved: bool,
    pub remaining_conflicts: usize,
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
        Ok(f(session))
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
}
