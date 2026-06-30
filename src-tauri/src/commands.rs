use crate::manager::{Launch, SessionManager};
use mcr_core::SessionModel;
use tauri::State;

type Mgr<'a> = State<'a, SessionManager>;

#[derive(serde::Serialize)]
pub struct Bootstrap {
    /// "merge" when launched as a Git mergetool, "demo" otherwise.
    pub mode: String,
    pub model: Option<SessionModel>,
}

/// First call from the UI: in mergetool mode this opens Git's files; otherwise
/// the UI falls back to its demo content.
#[tauri::command]
pub fn bootstrap(mgr: Mgr, launch: State<Launch>) -> Result<Bootstrap, String> {
    match &launch.merge {
        Some(files) => Ok(Bootstrap {
            mode: "merge".into(),
            model: Some(mgr.open_files(files)?),
        }),
        None => Ok(Bootstrap {
            mode: "demo".into(),
            model: None,
        }),
    }
}

/// Write the resolution back to Git's MERGED file.
#[tauri::command]
pub fn save_merged(mgr: Mgr, session_id: String) -> Result<(), String> {
    mgr.save_merged(&session_id)
}

/// Exit the process with a status Git interprets (0 = resolved, non-zero = abort).
#[tauri::command]
pub fn quit(code: i32) {
    std::process::exit(code);
}

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
