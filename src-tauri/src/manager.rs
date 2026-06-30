use mcr_core::{MergeSession, Side, SessionModel, WhitespaceMode};
use std::collections::HashMap;
use std::sync::Mutex;

/// Framework-agnostic session store. Holds all open merge sessions and forwards
/// intents to `mcr-core`. Kept free of Tauri types so it is unit-testable.
#[derive(Default)]
pub struct SessionManager {
    sessions: Mutex<HashMap<String, MergeSession>>,
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
}
