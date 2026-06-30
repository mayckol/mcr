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

/// How the app was launched: as a Git mergetool (with files) or standalone.
#[derive(Default)]
pub struct Launch {
    pub merge: Option<MergeFiles>,
}

/// Framework-agnostic session store. Holds all open merge sessions and forwards
/// intents to `mcr-core`. Kept free of Tauri types so it is unit-testable.
#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, MergeSession>>,
    merged_paths: Mutex<HashMap<String, String>>,
    counter: Mutex<u64>,
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
