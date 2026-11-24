//! Keep the account tray alive independently of the launcher window.

pub fn on_window_event<R: tauri::Runtime>(window: &tauri::Window<R>, event: &tauri::WindowEvent) {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if window.label() == "main" {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Err(error) = window.hide() {
                eprintln!("Multi Codex could not hide its window: {error}");
            }
        }
    }
}

pub fn on_run_event(event: &tauri::RunEvent) {
    // Closing the last native window can request process exit separately from
    // CloseRequested. The tray must survive that request. Explicit Quit and
    // updater restarts carry an exit code and must still be allowed through.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    if let tauri::RunEvent::ExitRequested {
        code: None, api, ..
    } = event
    {
        api.prevent_exit();
    }
}
