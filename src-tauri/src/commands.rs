use crate::discovery;
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
    /// Compare mode only: the ref the working tree is compared against.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare_ref: Option<String>,
}

/// First call from the UI. Runs the repository discovery (deferred from launch so
/// the window opens instantly) and registers the file set — sessions themselves
/// stay lazy and build on selection.
///
/// `(async)` runs it on the command thread pool: the scan must never block the
/// main thread, or the freshly-opened window freezes exactly when it should be
/// painting its loading state.
#[tauri::command(async)]
pub fn bootstrap(mgr: Mgr, launch: State<Launch>) -> Result<Bootstrap, String> {
    // A webview reload must not re-register the whole set — answer from state.
    if mgr.mark_booted() {
        let mode = if launch.compare_ref.is_some() {
            "compare"
        } else if launch.passed.is_some() {
            "merge"
        } else {
            "demo"
        };
        return Ok(Bootstrap {
            mode: mode.into(),
            files: mgr.summaries(),
            progress: mgr.progress(),
            active: None,
            file_name: None,
            compare_ref: launch.compare_ref.clone(),
        });
    }

    // Compare launch: no repo ctx (staging/backup stay inert) and no git-passed
    // file (exit code stays 0 — compare has no mergetool contract).
    if let (Some(root), Some(refspec)) = (&launch.repo_root, &launch.compare_ref) {
        let files = discovery::changed_paths(root, refspec)?;
        mgr.set_compare_ctx(root, refspec);
        // Register only — sessions build lazily on selection, so a launch with
        // hundreds of changed files stays instant.
        let ids: Vec<String> = files
            .iter()
            .map(|f| mgr.register_compare_entry(f))
            .collect();
        let mut single_model = None;
        let mut active_label = None;
        if files.len() == 1 {
            single_model = mgr.model(&ids[0]).ok();
            if single_model.is_some() {
                active_label = Some(files[0].path.clone());
            }
        }
        return Ok(Bootstrap {
            mode: "compare".into(),
            files: mgr.summaries(),
            progress: mgr.progress(),
            active: single_model,
            file_name: active_label.as_deref().and_then(basename),
            compare_ref: Some(refspec.clone()),
        });
    }

    let Some(passed) = &launch.passed else {
        return Ok(Bootstrap {
            mode: "demo".into(),
            files: Vec::new(),
            progress: mgr.progress(),
            active: None,
            file_name: None,
            compare_ref: None,
        });
    };

    // One `git ls-files -u` lists every conflicted path with its stages — no
    // per-file subprocesses, no worktree stat scan.
    let discovered = launch
        .repo_root
        .as_ref()
        .map(|r| discovery::unmerged_stage_sets(r).unwrap_or_default())
        .unwrap_or_default();

    // No repository context, or nothing discovered: legacy single-file behavior.
    let root = match &launch.repo_root {
        Some(r) if !discovered.is_empty() => r.clone(),
        _ => {
            let model = mgr.open_files(passed)?;
            mgr.set_git_passed_id(model.session_id.clone());
            return Ok(Bootstrap {
                mode: "merge".into(),
                files: Vec::new(),
                progress: mgr.progress(),
                active: Some(model),
                file_name: basename(&passed.merged),
                compare_ref: None,
            });
        }
    };

    mgr.set_repo(root.clone(), discovery::keep_backup(&root));
    let single = discovered.len() == 1;
    let mut single_model = None;
    let mut active_label = None;
    for (rel, stages) in &discovered {
        let id = mgr.register_merge_entry(&root, rel, *stages);
        if single {
            // A lone conflicted file opens straight into the editor (FR-015);
            // materialization can reveal binary, which stays accept-only.
            single_model = mgr.model(&id).ok();
            active_label = Some(rel.clone());
        }
    }
    mgr.set_git_passed_by_worktree(&passed.merged);
    Ok(Bootstrap {
        mode: "merge".into(),
        files: mgr.summaries(),
        progress: mgr.progress(),
        active: single_model,
        file_name: active_label.as_deref().and_then(basename),
        compare_ref: None,
    })
}

/// Re-fetch the file list + progress after state changes.
#[tauri::command(async)]
pub fn list_sessions(mgr: Mgr) -> (Vec<SessionSummary>, SessionProgress) {
    (mgr.summaries(), mgr.progress())
}

/// Load a specific file's model when the user selects it from the list.
#[tauri::command(async)]
pub fn select_session(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.model(&session_id)
}

/// Write + stage a resolved file (incremental persist).
#[tauri::command(async)]
pub fn save_and_stage(mgr: Mgr, session_id: String) -> Result<(), String> {
    mgr.save_and_stage(&session_id)
}

/// Resolve a whole file to one side directly from the list.
#[tauri::command(async)]
pub fn accept_file(
    mgr: Mgr,
    session_id: String,
    from: String,
) -> Result<SessionSummary, String> {
    mgr.accept_file(&session_id, &from)
}

/// Focus the next unresolved file after `current` (or the first if `None`).
#[tauri::command(async)]
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
#[tauri::command(async)]
pub fn finish(mgr: Mgr) -> Result<FinishOutcome, String> {
    mgr.finish()
}

/// Exit code Git should see for the file it is blocked on (0 = resolved).
#[tauri::command(async)]
pub fn exit_code(mgr: Mgr) -> i32 {
    mgr.exit_code()
}

/// Write the resolution back to Git's MERGED file.
#[tauri::command(async)]
pub fn save_merged(mgr: Mgr, session_id: String) -> Result<(), String> {
    mgr.save_merged(&session_id)
}

/// Exit the process with a status Git interprets (0 = resolved, non-zero = abort).
#[tauri::command]
pub fn quit(code: i32) {
    std::process::exit(code);
}

#[tauri::command(async)]
pub fn open_session(
    mgr: Mgr,
    local: String,
    ancestor: String,
    incoming: String,
    whitespace_mode: Option<String>,
) -> SessionModel {
    mgr.open(&local, &ancestor, &incoming, whitespace_mode.as_deref())
}

#[tauri::command(async)]
pub fn apply_change(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
    from: String,
) -> Result<SessionModel, String> {
    mgr.apply_change(&session_id, hunk_id, &from)
}

#[tauri::command(async)]
pub fn apply_both(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
    first: String,
) -> Result<SessionModel, String> {
    mgr.apply_both(&session_id, hunk_id, &first)
}

#[tauri::command(async)]
pub fn revert_change(
    mgr: Mgr,
    session_id: String,
    hunk_id: usize,
) -> Result<SessionModel, String> {
    mgr.revert_change(&session_id, hunk_id)
}

#[tauri::command(async)]
pub fn apply_non_conflicting(
    mgr: Mgr,
    session_id: String,
    from: String,
) -> Result<SessionModel, String> {
    mgr.apply_non_conflicting(&session_id, &from)
}

#[tauri::command(async)]
pub fn edit_result(
    mgr: Mgr,
    session_id: String,
    start: usize,
    end: usize,
    text: String,
) -> Result<SessionModel, String> {
    mgr.edit_result(&session_id, start, end, &text)
}

#[tauri::command(async)]
pub fn edit_full_result(
    mgr: Mgr,
    session_id: String,
    text: String,
) -> Result<SessionModel, String> {
    mgr.set_full_result(&session_id, &text)
}

#[tauri::command(async)]
pub fn undo(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.undo(&session_id)
}

#[tauri::command(async)]
pub fn redo(mgr: Mgr, session_id: String) -> Result<SessionModel, String> {
    mgr.redo(&session_id)
}

#[tauri::command(async)]
pub fn navigate(
    mgr: Mgr,
    session_id: String,
    direction: String,
    from_hunk: Option<usize>,
) -> Result<Option<usize>, String> {
    mgr.navigate(&session_id, &direction, from_hunk)
}

#[tauri::command(async)]
pub fn set_whitespace_mode(
    mgr: Mgr,
    session_id: String,
    mode: String,
) -> Result<SessionModel, String> {
    mgr.set_whitespace_mode(&session_id, &mode)
}
