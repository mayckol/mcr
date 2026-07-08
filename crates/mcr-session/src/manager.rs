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

/// How the app was launched. `passed` is the single file Git's current invocation
/// handed us (used for fallback and to identify the file Git is waiting on).
/// Discovery of the conflicted/changed set is deferred to `bootstrap`, so the
/// window opens before any repository scan runs.
#[derive(Default)]
pub struct Launch {
    pub passed: Option<MergeFiles>,
    pub repo_root: Option<String>,
    /// Set when launched as `mcr diff <ref>` — the validated refspec; the changed
    /// files are discovered in `bootstrap`.
    pub compare_ref: Option<String>,
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

#[derive(Clone)]
struct CompareCtx {
    root: String,
    refspec: String,
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
    /// Compare launch context + files whose sessions haven't been built yet.
    /// Sessions materialize on first selection — opening a big diff must not pay
    /// one `git show` + diff per file up front.
    compare_ctx: Mutex<Option<CompareCtx>>,
    compare_pending: Mutex<HashMap<String, discovery::ChangedFile>>,
    /// Merge entries registered from the batched stage listing whose sessions
    /// haven't been built yet — same lazy contract as compare_pending.
    merge_pending: Mutex<HashMap<String, discovery::StageSet>>,
    /// Guards against a second `bootstrap` (webview reload) re-registering the set.
    booted: Mutex<bool>,
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

    /// Whether a full bootstrap already ran (idempotence guard for webview reloads).
    /// Returns the previous value and marks the manager booted.
    pub fn mark_booted(&self) -> bool {
        let mut b = self.booted.lock().unwrap();
        std::mem::replace(&mut *b, true)
    }

    /// List one conflicted file WITHOUT any IO — no blob fetch, no diff. The kind
    /// is provisional (binary is only discoverable from content); the session
    /// materializes on first selection, exactly like compare entries.
    pub fn register_merge_entry(
        &self,
        root: &str,
        rel: &str,
        stages: discovery::StageSet,
    ) -> String {
        let worktree_path = std::path::Path::new(root)
            .join(rel)
            .to_string_lossy()
            .into_owned();
        let id = self.next_id();
        self.entries.lock().unwrap().insert(
            id.clone(),
            MergeFileEntry {
                path_label: rel.to_string(),
                worktree_path,
                kind: discovery::kind_from_stages(stages),
                resolved: false,
                accepted_raw: false,
                change_status: None,
            },
        );
        self.merge_pending.lock().unwrap().insert(id.clone(), stages);
        self.order.lock().unwrap().push(id.clone());
        id
    }

    /// Build the session for a lazily-registered merge file: reconstruct the three
    /// sides from the index stages, detect binary content (raw bytes must never
    /// round-trip through a lossy text session), and open the diff3 session.
    fn materialize_merge(&self, session_id: &str) -> Result<(), String> {
        if self.merge_pending.lock().unwrap().remove(session_id).is_none() {
            return Ok(()); // not a pending merge entry — nothing to build
        }
        let root = self
            .repo
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.root.clone())
            .ok_or_else(|| "no repository for merge session".to_string())?;
        let (label, worktree_path) = {
            let entries = self.entries.lock().unwrap();
            let e = entries
                .get(session_id)
                .ok_or_else(|| format!("unknown entry: {session_id}"))?;
            (e.path_label.clone(), e.worktree_path.clone())
        };
        let sides = discovery::reconstruct_raw_sides(&root, &label);
        if [&sides.base, &sides.local, &sides.incoming]
            .iter()
            .any(|b| discovery::blob_is_binary(b))
        {
            if let Some(e) = self.entries.lock().unwrap().get_mut(session_id) {
                e.kind = ConflictKind::Binary;
            }
            return Err("binary conflict — use Accept Ours / Theirs".to_string());
        }
        let text = |b: &[u8]| String::from_utf8_lossy(b).into_owned();
        let session = MergeSession::open(
            session_id.to_string(),
            &text(&sides.local),
            &text(&sides.base),
            &text(&sides.incoming),
            WhitespaceMode::None,
        );
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), session);
        self.merged_paths
            .lock()
            .unwrap()
            .insert(session_id.to_string(), worktree_path);
        Ok(())
    }

    /// Record the compare launch context (repo root + the ref compared against).
    pub fn set_compare_ctx(&self, root: &str, refspec: &str) {
        *self.compare_ctx.lock().unwrap() = Some(CompareCtx {
            root: root.to_string(),
            refspec: refspec.to_string(),
        });
    }

    /// List one compared file WITHOUT any IO — no blob fetch, no diff. The
    /// session materializes on first selection (`model`), so a launch with many
    /// changed files opens instantly.
    pub fn register_compare_entry(&self, file: &discovery::ChangedFile) -> String {
        let root = self
            .compare_ctx
            .lock()
            .unwrap()
            .as_ref()
            .map(|c| c.root.clone())
            .unwrap_or_default();
        let worktree_path = std::path::Path::new(&root)
            .join(&file.path)
            .to_string_lossy()
            .into_owned();
        let label = match &file.old_path {
            Some(old) => format!("{old} → {}", file.path),
            None => file.path.clone(),
        };
        let id = self.next_id();
        self.entries.lock().unwrap().insert(
            id.clone(),
            MergeFileEntry {
                path_label: label,
                worktree_path,
                kind: ConflictKind::Text,
                resolved: false,
                accepted_raw: false,
                change_status: Some(file.status.clone()),
            },
        );
        self.compare_pending
            .lock()
            .unwrap()
            .insert(id.clone(), file.clone());
        self.order.lock().unwrap().push(id.clone());
        id
    }

    /// Clear the compare view so a fresh single-file compare can open without
    /// leftover entries/sessions from a prior file. An embedding host that only
    /// ever compares drives this per file click; it holds no merge state, so
    /// clearing the shared maps wholesale is safe here.
    pub fn reset_compare(&self) {
        self.sessions.lock().unwrap().clear();
        self.merged_paths.lock().unwrap().clear();
        self.entries.lock().unwrap().clear();
        self.order.lock().unwrap().clear();
        self.compare_pending.lock().unwrap().clear();
        *self.compare_ctx.lock().unwrap() = None;
    }

    /// Open (or re-open) one file's compare session on demand — the runtime entry
    /// an embedding host drives in place of the CLI `Launch`/`bootstrap` path.
    /// Reuses the same lazy materialization (`ref` blob vs worktree) as a launch
    /// compare, so binary/non-UTF8 files surface the same error.
    pub fn open_compare_single(
        &self,
        root: &str,
        refspec: &str,
        path: &str,
    ) -> Result<SessionModel, String> {
        self.reset_compare();
        self.set_compare_ctx(root, refspec);
        let id = self.register_compare_entry(&discovery::ChangedFile {
            status: "M".to_string(),
            path: path.to_string(),
            old_path: None,
        });
        self.model(&id)
    }

    /// Build the session for a lazily-registered compare file: local = blob at
    /// the ref, base = incoming = the current worktree content — so the result
    /// projection IS the working file and every hunk is a place the ref differs
    /// from it (apply pulls the ref's version in). Binary (or non-UTF8) files
    /// flip their entry to `Binary` and never get a session.
    fn materialize_compare(&self, session_id: &str) -> Result<(), String> {
        if self.sessions.lock().unwrap().contains_key(session_id) {
            return Ok(());
        }
        let Some(file) = self.compare_pending.lock().unwrap().remove(session_id) else {
            return Ok(()); // not a compare entry — nothing to build
        };
        let ctx = self
            .compare_ctx
            .lock()
            .unwrap()
            .clone()
            .ok_or_else(|| "no compare context".to_string())?;

        let path_at_ref = file.old_path.as_deref().unwrap_or(&file.path);
        let ref_bytes = discovery::ref_blob(&ctx.root, &ctx.refspec, path_at_ref);
        let worktree_path = std::path::Path::new(&ctx.root)
            .join(&file.path)
            .to_string_lossy()
            .into_owned();
        let worktree_bytes = std::fs::read(&worktree_path).unwrap_or_default();

        if discovery::blob_is_binary(&ref_bytes) || discovery::blob_is_binary(&worktree_bytes) {
            if let Some(e) = self.entries.lock().unwrap().get_mut(session_id) {
                e.kind = ConflictKind::Binary;
            }
            return Err("binary file — cannot compare as text".to_string());
        }

        let current = String::from_utf8_lossy(&worktree_bytes).into_owned();
        // open_unapplied: the ref's changes start unresolved, so the editable
        // pane IS the current file until the user pulls a hunk in.
        let session = MergeSession::open_unapplied(
            session_id.to_string(),
            &String::from_utf8_lossy(&ref_bytes),
            &current,
            &current,
            WhitespaceMode::None,
        );
        self.sessions
            .lock()
            .unwrap()
            .insert(session_id.to_string(), session);
        self.merged_paths
            .lock()
            .unwrap()
            .insert(session_id.to_string(), worktree_path);
        Ok(())
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
            .map(|s| s.resolution_status().fully_resolved)
            .unwrap_or(false)
    }

    fn summary(&self, session_id: &str) -> Option<SessionSummary> {
        let entries = self.entries.lock().unwrap();
        let entry = entries.get(session_id)?;
        // remaining is None until the session materializes — an unopened file must
        // never read as resolved just because no session exists to count conflicts.
        let remaining = self
            .sessions
            .lock()
            .unwrap()
            .get(session_id)
            .map(|s| s.resolution_status().remaining_conflicts);
        let resolved =
            entry.resolved || (entry.kind == ConflictKind::Text && remaining == Some(0));
        let remaining = remaining.unwrap_or(0);
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

    /// Read-only model for a session (file selection). Lazily-registered compare
    /// and merge files build their session on first call.
    pub fn model(&self, session_id: &str) -> Result<SessionModel, String> {
        self.materialize_compare(session_id)?;
        self.materialize_merge(session_id)?;
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
            // A lazily-registered file may not have a session yet (whole-file accept
            // straight from the list); materializing can also reveal it is binary,
            // which routes to the raw-accept path below.
            if let Err(e) = self.materialize_merge(session_id) {
                let now_binary = self
                    .entries
                    .lock()
                    .unwrap()
                    .get(session_id)
                    .is_some_and(|e| e.kind == ConflictKind::Binary);
                if !now_binary {
                    return Err(e);
                }
                self.accept_special(session_id, from)?;
                return self
                    .summary(session_id)
                    .ok_or_else(|| format!("unknown entry: {session_id}"));
            }
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
        let mut discovered = discovery::unmerged_stage_sets(&root).unwrap();
        discovered.sort_by(|a, b| a.0.cmp(&b.0));
        let names: Vec<&str> = discovered.iter().map(|(p, _)| p.as_str()).collect();
        assert_eq!(names, vec!["a.txt", "b.txt"]);
        assert_eq!(
            discovery::kind_from_stages(discovered[0].1),
            ConflictKind::Text
        );

        let m = SessionManager::new();
        m.set_repo(root.clone(), false);
        let ids: Vec<String> = discovered
            .iter()
            .map(|(rel, stages)| m.register_merge_entry(&root, rel, *stages))
            .collect();

        // Registration is lazy: no sessions until a file is selected or accepted,
        // and unopened files must read unresolved.
        assert!(m.sessions.lock().unwrap().is_empty());
        let p0 = m.progress();
        assert_eq!(p0.total, 2);
        assert_eq!(p0.resolved_count, 0);
        m.model(&ids[0]).unwrap();
        assert_eq!(m.sessions.lock().unwrap().len(), 1);

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
        assert!(discovery::unmerged_stage_sets(&root).unwrap().is_empty());

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
    fn repo_root_dir_anchors_at_the_directory_itself() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-rootdir-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        setup_compare_repo(&dir);
        let anchored = discovery::repo_root_dir(&dir.to_string_lossy()).expect("repo root");
        // Canonicalize both sides: git prints the symlink-resolved toplevel
        // (/private/tmp vs /tmp on macOS).
        let canon = |p: &str| std::fs::canonicalize(p).unwrap();
        assert_eq!(canon(&anchored), canon(&dir.to_string_lossy()));
        // A dir OUTSIDE any repository anchors nowhere — the old file-oriented
        // path took the parent, which could accidentally be a repo.
        assert!(discovery::repo_root_dir("/").is_none());
        let _ = std::fs::remove_dir_all(&dir);
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

        // Worktree is the `main` checkout; statuses read ref → worktree:
        // added.txt exists only at `feature` (D), gone.txt only in the worktree (A).
        let mut files = discovery::changed_paths(&root, &feature).unwrap();
        files.sort_by(|a, b| a.path.cmp(&b.path));
        let statuses: Vec<(&str, &str)> = files
            .iter()
            .map(|f| (f.path.as_str(), f.status.as_str()))
            .collect();
        assert_eq!(
            statuses,
            vec![("added.txt", "D"), ("f.txt", "M"), ("gone.txt", "A")]
        );

        // Rename detection carries the old path (ref side).
        git_ok(&dir, &["checkout", "-q", "-b", "renamer"]);
        git_ok(&dir, &["mv", "f.txt", "renamed.txt"]);
        git_ok(&dir, &["commit", "-q", "-am", "rename"]);
        let renamed = discovery::changed_paths(&root, &main).unwrap();
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
        let (_main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();
        // Dirty the worktree so it differs from the ref.
        std::fs::write(dir.join("f.txt"), "one\nworktree\nthree\n").unwrap();

        let m = SessionManager::new();
        let f = discovery::ChangedFile {
            status: "M".into(),
            path: "f.txt".into(),
            old_path: None,
        };
        m.set_compare_ctx(&root, &feature);
        let id = m.register_compare_entry(&f);
        let model = m.model(&id).unwrap();
        // The editable pane starts as the CURRENT worktree content; the left pane
        // is the ref's version.
        assert_eq!(model.panes.result.join("\n"), "one\nworktree\nthree\n");
        assert_eq!(model.panes.local.join("\n"), "one\nfeature\nthree\n");

        // Pull the ref's version of the diverging hunk into the current file, save.
        let hunk = model
            .hunks
            .iter()
            .find(|h| h.origin == mcr_core::Origin::Local)
            .expect("ref differs from worktree");
        m.apply_change(&model.session_id, hunk.id, "local").unwrap();
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
        let _ = main;
        // added.txt exists only at `feature` — not in the main-checkout worktree.
        let added = discovery::ChangedFile {
            status: "D".into(),
            path: "added.txt".into(),
            old_path: None,
        };
        m.set_compare_ctx(&root, &feature);
        let model = m.model(&m.register_compare_entry(&added)).unwrap();
        assert_eq!(model.panes.local.join("\n"), "new\n");
        assert_eq!(model.panes.result.join("\n"), "");

        // gone.txt exists in the worktree but not at `feature`. Compare against
        // the actual on-disk bytes: on Windows CI, autocrlf checkouts write CRLF,
        // and the pane must mirror the worktree exactly either way.
        let gone = discovery::ChangedFile {
            status: "A".into(),
            path: "gone.txt".into(),
            old_path: None,
        };
        let model = m.model(&m.register_compare_entry(&gone)).unwrap();
        assert_eq!(model.panes.local.join("\n"), "");
        let on_disk = String::from_utf8(std::fs::read(dir.join("gone.txt")).unwrap()).unwrap();
        assert!(on_disk.starts_with("keep"), "on_disk = {on_disk:?}");
        assert_eq!(model.panes.result.join("\n"), on_disk);

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
            status: "D".into(),
            path: "blob.bin".into(),
            old_path: None,
        };
        m.set_compare_ctx(&root, "bin");
        let id = m.register_compare_entry(&f);
        let err = m.model(&id).unwrap_err();
        assert!(err.contains("binary"), "err = {err}");
        let summaries = m.summaries();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].kind, ConflictKind::Binary);
        assert_eq!(summaries[0].change_status.as_deref(), Some("D"));
        assert!(m.model(&summaries[0].session_id).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn compare_sessions_materialize_lazily() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-lazy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (_main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();

        let m = SessionManager::new();
        m.set_compare_ctx(&root, &feature);
        let ids: Vec<String> = ["f.txt", "added.txt", "gone.txt"]
            .iter()
            .map(|p| {
                m.register_compare_entry(&discovery::ChangedFile {
                    status: "M".into(),
                    path: p.to_string(),
                    old_path: None,
                })
            })
            .collect();

        // Registration lists all files but builds no sessions (no IO paid yet).
        assert_eq!(m.summaries().len(), 3);
        assert!(m.sessions.lock().unwrap().is_empty());

        // Selecting one file materializes exactly that session.
        m.model(&ids[0]).unwrap();
        assert_eq!(m.sessions.lock().unwrap().len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn open_compare_single_reopens_and_resets() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("mcr-cmp-single-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (_main, feature) = setup_compare_repo(&dir);
        let root = dir.to_string_lossy().into_owned();

        let m = SessionManager::new();
        // f.txt differs between the worktree (main) and `feature`.
        let model = m.open_compare_single(&root, &feature, "f.txt").unwrap();
        assert_eq!(model.panes.local.join("\n"), "one\nfeature\nthree\n");
        assert_eq!(m.summaries().len(), 1, "only the opened file is listed");

        // Re-opening another file resets — the prior entry/session is gone.
        let sid_before = model.session_id.clone();
        let next = m.open_compare_single(&root, &feature, "gone.txt").unwrap();
        assert_eq!(m.summaries().len(), 1);
        assert_ne!(next.session_id, sid_before);
        assert!(m.model(&sid_before).is_err(), "stale session dropped");

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
        let discovered = discovery::unmerged_stage_sets(&root).unwrap();
        assert_eq!(discovered.len(), 1);
        // Stages alone say Text; content check at materialization flips to Binary.
        let m = SessionManager::new();
        m.set_repo(root.clone(), false);
        let id = m.register_merge_entry(&root, &discovered[0].0, discovered[0].1);
        let err = m.model(&id).unwrap_err();
        assert!(err.contains("binary"), "err = {err}");
        assert_eq!(m.summaries()[0].kind, ConflictKind::Binary);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
