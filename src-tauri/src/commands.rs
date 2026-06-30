use crate::manager::SessionManager;
use mcr_core::SessionModel;
use tauri::State;

type Mgr<'a> = State<'a, SessionManager>;

#[tauri::command]
pub fn open_session(
    mgr: Mgr,
    local: String,
    ancestor: String,
    incoming: String,
    whitespace_mode: Option<String>,
) -> SessionModel {
    mgr.open(&local, &ancestor, &incoming, whitespace_mode.as_deref())
}

#[tauri::command]
pub fn apply_change(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
    from: String,
) -> Result<SessionModel, String> {
    mgr.apply_change(&session_id, hunk_id, &from)
}

#[tauri::command]
pub fn revert_change(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
) -> Result<SessionModel, String> {
    mgr.revert_change(&session_id, hunk_id)
}

#[tauri::command]
pub fn apply_non_conflicting(
    mgr: Mgr,
    session_id: String,
    from: String,
) -> Result<SessionModel, String> {
    mgr.apply_non_conflicting(&session_id, &from)
}

#[tauri::command]
pub fn edit_result(
    mgr: Mgr,
    session_id: String,
    start: usize,
    end: usize,
    text: String,
) -> Result<SessionModel, String> {
    mgr.edit_result(&session_id, start, end, &text)
}

#[tauri::command]
pub fn undo(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.undo(&session_id)
}

#[tauri::command]
pub fn redo(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.redo(&session_id)
}

#[tauri::command]
pub fn navigate(
    mgr: Mgr,
    session_id: String,
    direction: String,
    from_hunk: Option<usize>,
) -> Result<Option<usize>, String> {
    mgr.navigate(&session_id, &direction, from_hunk)
}

#[tauri::command]
pub fn set_whitespace_mode(
    mgr: Mgr,
    session_id: String,
    mode: String,
) -> Result<SessionModel, String> {
    mgr.set_whitespace_mode(&session_id, &mode)
}
