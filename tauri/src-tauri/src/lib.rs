mod desktop_integration;
mod profiles;
mod usage;

use desktop_integration::{DesktopIntegration, DesktopIntegrationStatus};
use profiles::{
    choose_workspace as choose_workspace_path, default_service, resolve_codex_command,
    validate_auth_structure, CodexCliRecognizer, KeyringSecretStore, ProfileRuntime,
    ProfileService, ProfileView, SaveProfileInput,
};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, State};
use usage::ProfileLimits;

type AppService = ProfileService<KeyringSecretStore, CodexCliRecognizer>;

struct AppState {
    service: Arc<AppService>,
    desktop_integration: DesktopIntegration,
    limit_checks: Arc<Mutex<HashSet<String>>>,
    device_logins: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
}

#[tauri::command]
fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    state.service.list_profiles()
}

#[tauri::command]
fn add_profile(input: SaveProfileInput, state: State<'_, AppState>) -> Result<ProfileView, String> {
    state.service.add_profile(input)
}

#[tauri::command]
fn import_current_profile(
    name: String,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    state.service.import_current(name, notes)
}

#[tauri::command]
fn update_profile(
    id: String,
    name: String,
    auth_json: Option<String>,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    state.service.update_profile(&id, name, auth_json, notes)
}

#[tauri::command]
fn validate_auth(auth_json: String, state: State<'_, AppState>) -> Result<String, String> {
    validate_auth_structure(&auth_json)?;
    profiles::AuthRecognizer::recognize(&CodexCliRecognizer, &auth_json)?;
    let _ = state;
    validate_auth_structure(&auth_json)
}

#[tauri::command]
fn choose_workspace() -> Result<Option<String>, String> {
    choose_workspace_path()
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DeviceLoginEvent {
    id: String,
    output: Option<String>,
    completed: bool,
    error: Option<String>,
}

fn emit_device_login(app: &tauri::AppHandle, event: DeviceLoginEvent) {
    let _ = app.emit("device-login", event);
}

#[tauri::command]
fn begin_device_login(
    name: String,
    notes: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let pending = state.service.prepare_device_login(name, notes)?;
    let id = pending.id.clone();
    let codex = match resolve_codex_command() {
        Ok(codex) => codex,
        Err(error) => {
            let _ = state.service.abandon_device_login(pending);
            return Err(error);
        }
    };
    let mut child = Command::new(codex)
        .args(["login", "--device-auth"])
        .env("CODEX_HOME", &pending.codex_home)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| {
            let _ = state.service.abandon_device_login(pending.clone());
            "Could not start the Codex browser sign-in".to_string()
        })?;
    let (cancel_sender, cancel_receiver) = mpsc::channel();
    {
        let mut logins = state
            .device_logins
            .lock()
            .map_err(|_| "Browser sign-in state is unavailable".to_string())?;
        logins.insert(id.clone(), cancel_sender);
    }
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let service = Arc::clone(&state.service);
    let active_logins = Arc::clone(&state.device_logins);
    let app_handle = app.clone();
    let output_id = id.clone();
    std::thread::spawn(move || {
        let emit_lines =
            |reader: Box<dyn std::io::Read + Send>, app: tauri::AppHandle, id: String| {
                std::thread::spawn(move || {
                    for line in BufReader::new(reader).lines().map_while(Result::ok) {
                        emit_device_login(
                            &app,
                            DeviceLoginEvent {
                                id: id.clone(),
                                output: Some(line),
                                completed: false,
                                error: None,
                            },
                        );
                    }
                })
            };
        if let Some(stdout) = stdout {
            emit_lines(Box::new(stdout), app_handle.clone(), output_id.clone());
        }
        if let Some(stderr) = stderr {
            emit_lines(Box::new(stderr), app_handle.clone(), output_id.clone());
        }
        let mut cancelled = false;
        let status = loop {
            if cancel_receiver.try_recv().is_ok() {
                cancelled = true;
                let _ = child.kill();
                break child.wait();
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(error) => break Err(error),
            }
        };
        let result = match status {
            _ if cancelled => {
                let _ = service.abandon_device_login(pending);
                Err("Browser sign-in was cancelled".to_string())
            }
            Ok(status) if status.success() => service.finish_device_login(pending),
            Ok(_) => {
                let _ = service.abandon_device_login(pending);
                Err("Browser sign-in was cancelled or did not complete".to_string())
            }
            Err(_) => {
                let _ = service.abandon_device_login(pending);
                Err("Could not wait for the Codex browser sign-in".to_string())
            }
        };
        if let Ok(mut logins) = active_logins.lock() {
            logins.remove(&output_id);
        }
        match result {
            Ok(_) => emit_device_login(
                &app_handle,
                DeviceLoginEvent {
                    id: output_id,
                    output: None,
                    completed: true,
                    error: None,
                },
            ),
            Err(error) => emit_device_login(
                &app_handle,
                DeviceLoginEvent {
                    id: output_id,
                    output: None,
                    completed: true,
                    error: Some(error),
                },
            ),
        }
    });
    Ok(id)
}

#[tauri::command]
fn cancel_device_login(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let sender = state
        .device_logins
        .lock()
        .map_err(|_| "Browser sign-in state is unavailable".to_string())?
        .remove(&id);
    if let Some(sender) = sender {
        let _ = sender.send(());
    }
    Ok(())
}

#[tauri::command]
fn launch_profile(id: String, workspace: String, state: State<'_, AppState>) -> Result<(), String> {
    state
        .service
        .launch_profile(&id, std::path::Path::new(&workspace))
}

#[tauri::command]
fn delete_profile(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.service.delete_profile(&id)
}

#[tauri::command]
fn get_runtime_status(id: String, state: State<'_, AppState>) -> Result<ProfileRuntime, String> {
    state.service.runtime_status(&id)
}

#[tauri::command]
async fn check_profile_limits(
    id: String,
    state: State<'_, AppState>,
) -> Result<ProfileLimits, String> {
    {
        let mut checks = state
            .limit_checks
            .lock()
            .map_err(|_| "Limits-check state is unavailable".to_string())?;
        if !checks.insert(id.clone()) {
            return Err("A limits check is already running for this profile".to_string());
        }
    }

    let service = Arc::clone(&state.service);
    let profile_id = id.clone();
    let result =
        tauri::async_runtime::spawn_blocking(move || service.check_profile_limits(&profile_id))
            .await;
    if let Ok(mut checks) = state.limit_checks.lock() {
        checks.remove(&id);
    }
    result.map_err(|_| "Codex limits check could not complete".to_string())?
}

#[tauri::command]
fn get_desktop_integration_status(
    state: State<'_, AppState>,
) -> Result<DesktopIntegrationStatus, String> {
    Ok(state.desktop_integration.status())
}

#[tauri::command]
fn install_desktop_integration(
    create_desktop_shortcut: bool,
    state: State<'_, AppState>,
) -> Result<DesktopIntegrationStatus, String> {
    state.desktop_integration.install(create_desktop_shortcut)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let service = default_service().unwrap_or_else(|error| {
        eprintln!("Multi Codex could not initialize: {error}");
        std::process::exit(1);
    });
    let desktop_integration = DesktopIntegration::discover().unwrap_or_else(|error| {
        eprintln!("Multi Codex could not initialize desktop integration: {error}");
        std::process::exit(1);
    });
    tauri::Builder::default()
        .manage(AppState {
            service: Arc::new(service),
            desktop_integration,
            limit_checks: Arc::new(Mutex::new(HashSet::new())),
            device_logins: Arc::new(Mutex::new(HashMap::new())),
        })
        .setup(|app| {
            use tauri::Manager;
            if let Some(window) = app.get_webview_window("main") {
                let icon = tauri::image::Image::from_bytes(include_bytes!("../icons/icon.png"))?;
                window.set_icon(icon)?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_profiles,
            add_profile,
            import_current_profile,
            update_profile,
            validate_auth,
            choose_workspace,
            begin_device_login,
            cancel_device_login,
            launch_profile,
            delete_profile,
            get_runtime_status,
            check_profile_limits,
            get_desktop_integration_status,
            install_desktop_integration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Multi Codex");
}
