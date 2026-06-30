mod commands;
mod discovery;
mod manager;

use manager::{DiscoveredFile, Launch, MergeFiles, SessionManager};

/// Parse Git's mergetool invocation: `mcr <LOCAL> <BASE> <REMOTE> <MERGED>`.
/// Flags (anything starting with `-`) are ignored so the positional contract holds.
///
/// Git hands MCR one file per invocation, so when launched inside a worktree we
/// discover the FULL conflicted set ourselves (research R1/R5); when launched
/// outside a worktree we fall back to the single file Git passed.
fn parse_launch() -> Launch {
    let paths: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    if paths.len() < 4 {
        return Launch::default();
    }
    let passed = MergeFiles {
        local: paths[0].clone(),
        base: paths[1].clone(),
        remote: paths[2].clone(),
        merged: paths[3].clone(),
    };
    match discovery::repo_root(&passed.merged) {
        Some(root) => {
            let keep_backup = discovery::keep_backup(&root);
            let files = discovery::unmerged_paths(&root)
                .unwrap_or_default()
                .into_iter()
                .map(|rel| DiscoveredFile {
                    worktree_path: std::path::Path::new(&root)
                        .join(&rel)
                        .to_string_lossy()
                        .into_owned(),
                    path_label: rel,
                })
                .collect();
            Launch {
                passed: Some(passed),
                repo_root: Some(root),
                files,
                keep_backup,
            }
        }
        None => Launch {
            passed: Some(passed),
            repo_root: None,
            files: Vec::new(),
            keep_backup: false,
        },
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SessionManager::new())
        .manage(parse_launch())
        // Closing the window with the native control is an abort, not a save: exit
        // with the per-file code (non-zero when the file Git passed is unresolved)
        // so Git never marks an unresolved file resolved. Explicit Save & Exit calls
        // `quit(0)`, which exits before this fires.
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                use tauri::Manager;
                let is_merge = window.state::<Launch>().passed.is_some();
                let resolved = window.state::<SessionManager>().git_passed_resolved();
                // Merge launch: abort (non-zero) unless the passed file is resolved,
                // so closing never stages unresolved content. Non-merge: clean exit.
                let code = if is_merge && !resolved { 1 } else { 0 };
                std::process::exit(code);
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::list_sessions,
            commands::select_session,
            commands::open_session,
            commands::apply_change,
            commands::apply_both,
            commands::revert_change,
            commands::apply_non_conflicting,
            commands::edit_result,
            commands::edit_full_result,
            commands::undo,
            commands::redo,
            commands::navigate,
            commands::set_whitespace_mode,
            commands::save_merged,
            commands::save_and_stage,
            commands::accept_file,
            commands::next_unresolved,
            commands::finish,
            commands::exit_code,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCR merge editor");
}
