//! Native menu bar uses the same account and launch flow as the main window.
use crate::{launch_targets::LaunchTargets, AppState};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::{Emitter, Manager};

pub fn launch_actions(targets: &LaunchTargets) -> Vec<(&'static str, &'static str)> {
    let mut actions = Vec::new();
    if targets.vscode_installed {
        actions.push(("vscode", "Open VS Code"));
    }
    if targets.standalone_installed && targets.standalone_verified {
        actions.push(("standalone", "Open Codex"));
    }
    if targets.codex_cli_available {
        actions.push(("cli", "Open CLI"));
    }
    actions
}

pub fn menu(app: &tauri::AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let menu = Menu::new(app)?;
    menu.append(&MenuItem::with_id(
        app,
        "open",
        "Open Multi Codex",
        true,
        None::<&str>,
    )?)?;
    let actions = launch_actions(&crate::launch_targets::detect());
    let service = app.state::<AppState>();
    let profiles = service.service.list_profiles().unwrap_or_default();
    if profiles.is_empty() {
        menu.append(&MenuItem::with_id(
            app,
            "add",
            "Add account…",
            true,
            None::<&str>,
        )?)?;
    }
    for profile in profiles {
        let account = Submenu::new(app, &profile.metadata.name, !actions.is_empty())?;
        for (target, label) in &actions {
            account.append(&MenuItem::with_id(
                app,
                format!("launch:{target}:{}", profile.metadata.id),
                *label,
                true,
                None::<&str>,
            )?)?;
        }
        menu.append(&account)?;
    }
    menu.append(&PredefinedMenuItem::separator(app)?)?;
    menu.append(&MenuItem::with_id(
        app,
        "refresh",
        "Refresh All Usage",
        true,
        None::<&str>,
    )?)?;
    menu.append(&MenuItem::with_id(
        app,
        "quit",
        "Quit Multi Codex",
        true,
        None::<&str>,
    )?)?;
    Ok(menu)
}

pub fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.maximize();
        let _ = window.set_focus();
    }
}

pub fn setup(app: &tauri::AppHandle) -> tauri::Result<()> {
    let initial_menu = menu(app)?;
    // A monochrome template adapts to both light and dark macOS menu bars.
    let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/32x32.png"))?;
    tauri::tray::TrayIconBuilder::with_id("accounts")
        .icon(icon)
        .icon_as_template(true)
        .tooltip("Multi Codex")
        .menu(&initial_menu)
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            match id {
                "quit" => app.exit(0),
                "open" | "add" => {
                    show(app);
                    if id == "add" {
                        let _ = app.emit("account-menu-add", ());
                    }
                }
                "refresh" => {
                    let _ = app.emit("account-menu-refresh", ());
                }
                _ => {
                    if let Some(value) = id.strip_prefix("launch:") {
                        if let Some((target, account)) = value.split_once(':') {
                            show(app);
                            let _ = app.emit(
                                "account-menu-launch",
                                serde_json::json!({"target":target,"id":account}),
                            );
                        }
                    }
                }
            }
        })
        .build(app)?;
    // Update after account edits in either the app or extension, and after apps
    // are installed/removed. Build menus only when the visible model changes.
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut previous = String::new();
        loop {
            let state = handle.state::<AppState>();
            let profiles = state.service.list_profiles().unwrap_or_default();
            let fingerprint = format!(
                "{:?}:{:?}",
                profiles
                    .iter()
                    .map(|p| (&p.metadata.id, &p.metadata.name))
                    .collect::<Vec<_>>(),
                launch_actions(&crate::launch_targets::detect())
            );
            if fingerprint != previous {
                if let Ok(menu) = menu(&handle) {
                    if let Some(tray) = handle.tray_by_id("accounts") {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
                previous = fingerprint;
            }
            std::thread::sleep(std::time::Duration::from_secs(5));
        }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn account_actions_only_include_installed_verified_targets() {
        let mut targets = LaunchTargets {
            platform: "macos".into(),
            vscode_installed: false,
            codex_cli_available: false,
            standalone_installed: false,
            standalone_verified: false,
            standalone_version: None,
        };
        assert!(launch_actions(&targets).is_empty());
        targets.vscode_installed = true;
        targets.standalone_installed = true;
        assert_eq!(launch_actions(&targets), [("vscode", "Open VS Code")]);
        targets.standalone_verified = true;
        targets.codex_cli_available = true;
        assert_eq!(launch_actions(&targets).len(), 3);
        targets.standalone_installed = false;
        assert_eq!(launch_actions(&targets).len(), 2);
    }
}
