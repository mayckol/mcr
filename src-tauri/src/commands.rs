use crate::manager::{FinishOutcome, Launch, SessionManager, SessionProgress, SessionSummary};
use mcr_core::SessionModel;
use tauri::State;

type Mgr<'a> = State<'a, SessionManager>;

fn basename(path: &str) -> Option<String> {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
}

#[derive(serde::Serialize)]
pub struct Bootstrap {
    /// "merge" when launched as a Git mergetool, "demo" otherwise.
    pub mode: String,
    /// The conflicted file list (empty in demo / single-file fallback).
    pub files: Vec<SessionSummary>,
    pub progress: SessionProgress,
    /// The model to render immediately: the single file when there is exactly one,
    /// otherwise `None` so the list is shown first (FR-001/FR-015).
    pub active: Option<SessionModel>,
    /// Basename of the active file, so the UI can pick a syntax highlighter.
    pub file_name: Option<String>,
}

/// First call from the UI. Discovers the conflicted set and opens the session(s).
#[tauri::command]
pub fn bootstrap(mgr: Mgr, launch: State<Launch>) -> Result<Bootstrap, String> {
    let Some(passed) = &launch.passed else {
        return Ok(Bootstrap {
            mode: "demo".into(),
            files: Vec::new(),
            progress: mgr.progress(),
            active: None,
            file_name: None,
        });
    };

    // No repository context, or nothing discovered: legacy single-file behavior.
    let root = match &launch.repo_root {
        Some(r) if !launch.files.is_empty() => r.clone(),
        _ => {
            let model = mgr.open_files(passed)?;
            mgr.set_git_passed_id(model.session_id.clone());
            return Ok(Bootstrap {
                mode: "merge".into(),
                files: Vec::new(),
                progress: mgr.progress(),
                active: Some(model),
                file_name: basename(&passed.merged),
            });
        }
    };

    mgr.set_repo(root.clone(), launch.keep_backup);
    let mut single_model = None;
    let single = launch.files.len() == 1;
    let mut active_label = None;
    for file in &launch.files {
        let model = mgr.open_entry(&root, file);
        if single {
            active_label = Some(file.path_label.clone());
            single_model = Some(model);
        }
    }
    mgr.set_git_passed_by_worktree(&passed.merged);
    Ok(Bootstrap {
        mode: "merge".into(),
        files: mgr.summaries(),
        progress: mgr.progress(),
        active: single_model,
        file_name: active_label.as_deref().and_then(basename),
    })
}

/// Re-fetch the file list + progress after state changes.
#[tauri::command]
pub fn list_sessions(mgr: Mgr) -> (Vec<SessionSummary>, SessionProgress) {
    (mgr.summaries(), mgr.progress())
}

/// Load a specific file's model when the user selects it from the list.
#[tauri::command]
pub fn select_session(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.model(&session_id)
}

/// Write + stage a resolved file (incremental persist).
#[tauri::command]
pub fn save_and_stage(mgr: Mgr, session_id: String) -> Result<(), String> {
    mgr.save_and_stage(&session_id)
}

/// Resolve a whole file to one side directly from the list.
#[tauri::command]
pub fn accept_file(
    mgr: Mgr,
    session_id: String,
    from: String,
) -> Result<SessionSummary, String> {
    mgr.accept_file(&session_id, &from)
}

/// Focus the next unresolved file after `current` (or the first if `None`).
#[tauri::command]
pub fn next_unresolved(mgr: Mgr, current: Option<String>) -> Option<String> {
    let order = mgr.unresolved_order();
    match current {
        Some(cur) => order
            .iter()
            .position(|id| *id == cur)
            .and_then(|i| order.get(i + 1).or_else(|| order.first()))
            .cloned()
            .or_else(|| order.first().cloned()),
        None => order.first().cloned(),
    }
}

/// Stage all resolved files and report whether the whole merge is done.
#[tauri::command]
pub fn finish(mgr: Mgr) -> Result<FinishOutcome, String> {
    mgr.finish()
}

/// Exit code Git should see for the file it is blocked on (0 = resolved).
#[tauri::command]
pub fn exit_code(mgr: Mgr) -> i32 {
    mgr.exit_code()
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
pub fn apply_both(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
    first: String,
) -> Result<SessionModel, String> {
    mgr.apply_both(&session_id, hunk_id, &first)
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
pub fn edit_full_result(
    mgr: Mgr,
    session_id: String,
    text: String,
) -> Result<SessionModel, String> {
    mgr.set_full_result(&session_id, &text)
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
