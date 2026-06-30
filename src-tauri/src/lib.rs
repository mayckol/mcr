mod commands;
mod manager;

use manager::SessionManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(SessionManager::new())
        .invoke_handler(tauri::generate_handler![
            commands::open_session,
            commands::apply_change,
            commands::revert_change,
            commands::apply_non_conflicting,
            commands::edit_result,
            commands::undo,
            commands::redo,
            commands::navigate,
            commands::set_whitespace_mode,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MCR merge editor");
}
