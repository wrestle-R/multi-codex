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

// AppKit's Dock Quit and Command-Q call NSApplication.terminate directly,
// bypassing Tauri's ExitRequested event. Cancel that native termination and
// hide the launcher. The tray's explicit app.exit(0) and updater restart use
// Tauri's event loop instead, so they still terminate normally.
#[cfg(target_os = "macos")]
pub fn install_native_quit_handler(app: &tauri::AppHandle) {
    use std::ffi::{c_char, c_void};
    use std::sync::OnceLock;
    use tauri::Manager;
    static APP: OnceLock<tauri::AppHandle> = OnceLock::new();
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn object_getClass(object: *mut c_void) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
        fn class_addMethod(
            class: *mut c_void,
            selector: *mut c_void,
            implementation: *const c_void,
            types: *const c_char,
        ) -> bool;
    }
    extern "C" fn should_terminate(_: *mut c_void, _: *mut c_void, _: *mut c_void) -> usize {
        if let Some(app) = APP.get() {
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = window.hide() {
                    eprintln!("Multi Codex could not hide its window: {error}");
                }
            }
        }
        0 // NSTerminateCancel
    }
    if APP.set(app.clone()).is_err() {
        return;
    }
    // Called on the main thread at Ready, after Tao installs its delegate.
    unsafe {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let native_app = send(
            objc_getClass(c"NSApplication".as_ptr()),
            sel_registerName(c"sharedApplication".as_ptr()),
        );
        let delegate = send(native_app, sel_registerName(c"delegate".as_ptr()));
        assert!(!delegate.is_null(), "AppKit delegate is missing");
        assert!(
            class_addMethod(
                object_getClass(delegate),
                sel_registerName(c"applicationShouldTerminate:".as_ptr()),
                should_terminate as *const c_void,
                c"Q@:@".as_ptr()
            ),
            "AppKit quit handler already exists"
        );
    }
}
