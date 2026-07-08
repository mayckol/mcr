mod commands;

use mcr_session::discovery;
use mcr_session::manager::{Launch, MergeFiles, SessionManager};

/// What the command line asked for. Stable CLI contract (editors/IDEs drive it):
/// `mcr <LOCAL> <BASE> <REMOTE> <MERGED>` (git mergetool) or
/// `mcr diff <ref> [dir]` (compare a branch/commit against the working tree;
/// `dir` anchors the repo for launchers that cannot preserve the caller's CWD,
/// e.g. the AppImage wrapper).
enum ParsedArgs {
    Mergetool(MergeFiles),
    Compare { refspec: String, dir: Option<String> },
    CompareUsage,
    Demo,
}

fn classify_args(args: &[String]) -> ParsedArgs {
    if args.first().map(String::as_str) == Some("diff") {
        let rest = &args[1..];
        return match rest.len() {
            1 | 2 => ParsedArgs::Compare {
                refspec: rest[0].clone(),
                dir: rest.get(1).cloned(),
            },
            _ => ParsedArgs::CompareUsage,
        };
    }
    // Mergetool contract: flags (anything starting with `-`) are ignored so the
    // positional contract holds.
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with('-')).collect();
    if paths.len() < 4 {
        return ParsedArgs::Demo;
    }
    ParsedArgs::Mergetool(MergeFiles {
        local: paths[0].clone(),
        base: paths[1].clone(),
        remote: paths[2].clone(),
        merged: paths[3].clone(),
    })
}

/// Resolve a compare launch or exit(2) with a usage error on stderr — argument
/// errors must never open a window (scriptable contract). Only the cheap
/// metadata checks run here (`rev-parse` — no worktree scan); the changed-file
/// discovery is deferred to `bootstrap` so the window opens instantly.
fn compare_launch(refspec: String, dir: Option<String>) -> Launch {
    let usage = "usage: mcr diff <branch|commit> [dir]";
    let root = match &dir {
        Some(d) => discovery::repo_root_dir(d),
        None => discovery::repo_root_cwd(),
    };
    let Some(root) = root else {
        eprintln!("mcr diff: not inside a git repository\n{usage}");
        std::process::exit(2);
    };
    if !discovery::resolves_to_commit(&root, &refspec) {
        eprintln!("mcr diff: '{refspec}' does not resolve to a commit\n{usage}");
        std::process::exit(2);
    }
    Launch {
        passed: None,
        repo_root: Some(root),
        compare_ref: Some(refspec),
    }
}

/// Parse the invocation. Git hands MCR one file per mergetool run, so when
/// launched inside a worktree `bootstrap` discovers the FULL conflicted set
/// (research R1/R5); outside a worktree it falls back to the single file Git
/// passed. Only the repo-root anchor is resolved here — everything that scales
/// with repository size waits until the window is up.
fn parse_launch() -> Launch {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let passed = match classify_args(&args) {
        ParsedArgs::Demo => return Launch::default(),
        ParsedArgs::CompareUsage => {
            eprintln!("usage: mcr diff <branch|commit> [dir]");
            std::process::exit(2);
        }
        ParsedArgs::Compare { refspec, dir } => return compare_launch(refspec, dir),
        ParsedArgs::Mergetool(files) => files,
    };
    let repo_root = discovery::repo_root(&passed.merged);
    Launch {
        passed: Some(passed),
        repo_root,
        compare_ref: None,
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    #[allow(unused_mut)]
    let mut builder = tauri::Builder::default();
    // macOS: add "Settings…" (Cmd+,) to the app menu, in the native position
    // right below About. It emits an event the webview turns into the settings
    // panel. Other platforms have no default app menu — the toolbar gear and
    // Ctrl+, cover them.
    #[cfg(target_os = "macos")]
    {
        use tauri::menu::{Menu, MenuItemBuilder, PredefinedMenuItem};
        use tauri::Emitter;
        builder = builder
            .menu(|handle| {
                let menu = Menu::default(handle)?;
                if let Some(app_menu) = menu.items()?.first().and_then(|i| i.as_submenu().cloned()) {
                    let settings = MenuItemBuilder::with_id("mcr-settings", "Settings…")
                        .accelerator("Cmd+,")
                        .build(handle)?;
                    let sep = PredefinedMenuItem::separator(handle)?;
                    // Default app menu: [About, separator, Services, ...] — slot in
                    // after About's separator.
                    app_menu.insert_items(&[&sep, &settings], 2)?;
                }
                Ok(menu)
            })
            .on_menu_event(|app, event| {
                if event.id() == "mcr-settings" {
                    let _ = app.emit("mcr://open-settings", ());
                }
            });
    }
    builder
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
            commands::compare_open,
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
        .build(tauri::generate_context!())
        .expect("error while building MCR merge editor")
        // Cmd+Q (macOS app-menu quit) and other app-level exits never reach the
        // window's CloseRequested handler; without this they'd fall through to a
        // clean status-0 exit and Git would stage an unresolved file.
        .run(|app, event| {
            use tauri::Manager;
            match event {
                // Spawned as a plain child of another app (fftracking, git),
                // macOS won't activate us — the window can open BEHIND the
                // caller and look like it never launched. Claim focus once ready.
                tauri::RunEvent::Ready => {
                    if let Some(w) = app.webview_windows().values().next() {
                        let _ = w.set_focus();
                    }
                }
                tauri::RunEvent::ExitRequested { .. } => {
                    let is_merge = app.state::<Launch>().passed.is_some();
                    let resolved = app.state::<SessionManager>().git_passed_resolved();
                    let code = if is_merge && !resolved { 1 } else { 0 };
                    std::process::exit(code);
                }
                _ => {}
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(args: &[&str]) -> Vec<String> {
        args.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn classify_diff_one_ref() {
        match classify_args(&v(&["diff", "main"])) {
            ParsedArgs::Compare { refspec, dir } => {
                assert_eq!(refspec, "main");
                assert!(dir.is_none());
            }
            _ => panic!("expected Compare"),
        }
    }

    #[test]
    fn classify_diff_with_dir_anchor() {
        match classify_args(&v(&["diff", "abc123", "/repo"])) {
            ParsedArgs::Compare { refspec, dir } => {
                assert_eq!(refspec, "abc123");
                assert_eq!(dir.as_deref(), Some("/repo"));
            }
            _ => panic!("expected Compare"),
        }
    }

    #[test]
    fn classify_diff_wrong_arity_is_usage() {
        assert!(matches!(classify_args(&v(&["diff"])), ParsedArgs::CompareUsage));
        assert!(matches!(
            classify_args(&v(&["diff", "a", "b", "c"])),
            ParsedArgs::CompareUsage
        ));
    }

    #[test]
    fn classify_mergetool_four_paths_with_flags() {
        match classify_args(&v(&["--flag", "L", "B", "R", "M"])) {
            ParsedArgs::Mergetool(f) => {
                assert_eq!(f.local, "L");
                assert_eq!(f.merged, "M");
            }
            _ => panic!("expected Mergetool"),
        }
    }

    #[test]
    fn classify_too_few_paths_is_demo() {
        assert!(matches!(classify_args(&v(&[])), ParsedArgs::Demo));
        assert!(matches!(classify_args(&v(&["a", "b"])), ParsedArgs::Demo));
    }
}
