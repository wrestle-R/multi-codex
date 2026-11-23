//! Native event-loop regression check, run on macOS CI with disposable state.
#[path = "../src/lifecycle.rs"]
mod lifecycle;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tauri::Manager;

#[derive(Default)]
struct Evidence {
    hidden: bool,
    reopened: bool,
    automatic_exit: bool,
    explicit_quit: bool,
    completed: bool,
}

fn main() {
    let without_guard = std::env::args().any(|arg| arg == "--without-exit-guard");
    let evidence = Arc::new(Mutex::new(Evidence::default()));
    let observed = Arc::clone(&evidence);
    let app = tauri::Builder::default()
        .on_window_event(lifecycle::on_window_event)
        .setup(|app| {
            tauri::WebviewWindowBuilder::new(app, "main", tauri::WebviewUrl::default())
                .title("Multi Codex close-to-tray regression")
                .build()?;
            tauri::tray::TrayIconBuilder::with_id("accounts")
                .icon(tauri::image::Image::from_bytes(include_bytes!(
                    "../icons/menu-bar.png"
                ))?)
                .icon_as_template(cfg!(target_os = "macos"))
                .build(app)?;
            Ok(())
        })
        .build(tauri::test::mock_context(tauri::test::noop_assets()))
        .expect("build native lifecycle fixture");
    let started = Instant::now();
    let mut phase = 0;
    let mut automatic_exit_at = None;
    let code = app.run_return(move |app, event| {
        if !without_guard {
            lifecycle::on_run_event(&event);
        }
        match event {
            tauri::RunEvent::Ready => {
                app.get_webview_window("main").unwrap().close().unwrap();
            }
            tauri::RunEvent::ExitRequested { code: None, .. } => {
                observed.lock().unwrap().automatic_exit = true;
                automatic_exit_at = Some(Instant::now());
                // Wake a later iteration so a pending exit cannot accidentally
                // pass on the final MainEventsCleared before LoopDestroyed.
                let handle = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(250));
                    let _ = handle.run_on_main_thread(|| {});
                });
            }
            tauri::RunEvent::ExitRequested { code: Some(0), .. } => {
                observed.lock().unwrap().explicit_quit = true;
            }
            tauri::RunEvent::MainEventsCleared => {
                assert!(
                    started.elapsed() < Duration::from_secs(20),
                    "lifecycle check timed out"
                );
                assert!(
                    app.tray_by_id("accounts").is_some(),
                    "account tray was removed"
                );
                match phase {
                    0 => {
                        let window = app
                            .get_webview_window("main")
                            .expect("close destroyed main window");
                        if !window.is_visible().unwrap() {
                            observed.lock().unwrap().hidden = true;
                            window.show().unwrap();
                            phase = 1;
                        }
                    }
                    1 => {
                        let window = app.get_webview_window("main").unwrap();
                        if window.is_visible().unwrap() {
                            observed.lock().unwrap().reopened = true;
                            // Exercise the separate last-window ExitRequested path,
                            // which a CloseRequested listener cannot intercept.
                            window.destroy().unwrap();
                            phase = 2;
                        }
                    }
                    2 if automatic_exit_at
                        .is_some_and(|at| at.elapsed() >= Duration::from_millis(200)) =>
                    {
                        observed.lock().unwrap().completed = true;
                        phase = 3;
                        app.exit(0);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    });
    let evidence = evidence.lock().unwrap();
    assert!(evidence.hidden, "close did not hide the main window");
    assert!(evidence.reopened, "hidden main window did not reopen");
    assert!(
        evidence.automatic_exit,
        "last-window exit path was not exercised"
    );
    assert!(
        evidence.completed,
        "automatic last-window exit stopped the tray process"
    );
    assert!(evidence.explicit_quit, "explicit Quit was blocked");
    assert_eq!(code, 0);
    println!(
        "PASS: close hides, reopen works, tray survives last-window exit, explicit Quit exits"
    );
}
