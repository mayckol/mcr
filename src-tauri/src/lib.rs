mod commands;
mod manager;

use manager::{Launch, MergeFiles, SessionManager};

/// Parse Git's mergetool invocation: `mcr <LOCAL> <BASE> <REMOTE> <MERGED>`.
/// Flags (anything starting with `-`) are ignored so the positional contract holds.
fn parse_launch() -> Launch {
    let paths: Vec<String> = std::env::args()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    if paths.len() >= 4 {
        Launch {
            merge: Some(MergeFiles {
                local: paths[0].clone(),
                base: paths[1].clone(),
                remote: paths[2].clone(),
                merged: paths[3].clone(),
            }),
        }
    } else {
        Launch { merge: None }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SessionManager::new())
        .manage(parse_launch())
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::open_session,
            commands::apply_change,
            commands::revert_change,
            commands::apply_non_conflicting,
            commands::edit_result,
            commands::undo,
            commands::redo,
            commands::navigate,
            commands::set_whitespace_mode,
            commands::save_merged,
            commands::quit,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCR merge editor");
}
