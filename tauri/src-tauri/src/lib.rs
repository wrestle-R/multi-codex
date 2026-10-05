#[cfg(target_os = "linux")]
mod application_icons;
mod desktop_environment;
mod desktop_integration;
mod launch;
mod launch_targets;
#[cfg(target_os = "linux")]
mod linux_desktops;
mod process;
mod profiles;
mod settings;
mod usage;

use desktop_integration::{DesktopIntegration, DesktopIntegrationStatus};
use profiles::{
    default_service, resolve_codex_command, validate_auth_structure, CodexCliRecognizer,
    KeyringSecretStore, ProfileRuntime, ProfileService, ProfileView, SaveProfileInput,
    StorageUsage,
};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::{Emitter, State};
use usage::ProfileLimits;

type AppService = ProfileService<KeyringSecretStore, CodexCliRecognizer>;

struct AppState {
    service: Arc<AppService>,
    launches: Arc<Mutex<launch::LaunchCoordinator>>,
    desktop_integration: DesktopIntegration,
    limit_checks: Arc<Mutex<HashSet<String>>>,
    device_logins: Arc<Mutex<HashMap<String, mpsc::Sender<()>>>>,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchEnvironment {
    default_workspace: String,
    capabilities: desktop_environment::DesktopCapabilities,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDirectory {
    name: String,
    path: String,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceDirectoryListing {
    path: String,
    parent_path: Option<String>,
    directories: Vec<WorkspaceDirectory>,
}

fn read_workspace_directories(requested_path: &str) -> Result<WorkspaceDirectoryListing, String> {
    let path = Path::new(requested_path)
        .canonicalize()
        .map_err(|error| format!("Could not open this folder: {error}"))?;
    if !path.is_dir() {
        return Err("The selected path is not a folder".to_string());
    }
    let mut directories = std::fs::read_dir(&path)
        .map_err(|error| format!("Could not read this folder: {error}"))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if !file_type.is_dir() {
                return None;
            }
            let entry_path = entry.path().canonicalize().ok()?;
            Some(WorkspaceDirectory {
                name: entry.file_name().to_string_lossy().into_owned(),
                path: entry_path.to_string_lossy().into_owned(),
            })
        })
        .collect::<Vec<_>>();
    directories.sort_by_key(|entry| entry.name.to_lowercase());
    Ok(WorkspaceDirectoryListing {
        path: path.to_string_lossy().into_owned(),
        parent_path: path
            .parent()
            .map(|parent| parent.to_string_lossy().into_owned()),
        directories,
    })
}

#[tauri::command]
async fn list_workspace_directories(path: String) -> Result<WorkspaceDirectoryListing, String> {
    tauri::async_runtime::spawn_blocking(move || read_workspace_directories(&path))
        .await
        .map_err(|_| "Could not read folders".to_string())?
}

#[tauri::command]
async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<ProfileView>, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.list_profiles())
        .await
        .map_err(|_| "Could not load profiles".to_string())?
}

#[tauri::command]
async fn add_profile(
    input: SaveProfileInput,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.add_profile(input))
        .await
        .map_err(|_| "Could not add the profile".to_string())?
}

#[tauri::command]
async fn import_current_profile(
    name: String,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.import_current(name, notes))
        .await
        .map_err(|_| "Could not import the current profile".to_string())?
}

#[tauri::command]
async fn update_profile(
    id: String,
    name: String,
    auth_json: Option<String>,
    notes: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || {
        service.update_profile(&id, name, auth_json, notes)
    })
    .await
    .map_err(|_| "Could not update the profile".to_string())?
}

#[tauri::command]
async fn validate_auth(auth_json: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_auth_structure(&auth_json)?;
        profiles::AuthRecognizer::recognize(&CodexCliRecognizer, &auth_json)?;
        validate_auth_structure(&auth_json)
    })
    .await
    .map_err(|_| "Could not validate the credential".to_string())?
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
async fn open_device_login_browser() -> Result<bool, String> {
    #[cfg(target_os = "macos")]
    {
        tauri::async_runtime::spawn_blocking(|| {
            let output = process::output(
                Command::new("/usr/bin/open").arg("https://auth.openai.com/codex/device"),
                Duration::from_secs(5),
            )?;
            if !output.status.success() {
                return Err("Could not open the default browser. Copy the sign-in link and open it manually.".into());
            }
            Ok(true)
        }).await.map_err(|_| "Could not open the default browser".to_string())?
    }
    #[cfg(not(target_os = "macos"))]
    Ok(false)
}

#[tauri::command]
fn begin_device_login(
    name: String,
    notes: Option<String>,
    profile_id: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let pending = if let Some(profile_id) = profile_id {
        state
            .service
            .prepare_profile_reauthentication(&profile_id)?
    } else {
        state.service.prepare_device_login(name, notes)?
    };
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
async fn get_executable_settings() -> Result<settings::ExecutableSettings, String> {
    tauri::async_runtime::spawn_blocking(settings::load)
        .await
        .map_err(|_| "Could not read settings".to_string())?
}

#[tauri::command]
async fn save_executable_settings(
    settings: settings::ExecutableSettings,
) -> Result<settings::ExecutableSettings, String> {
    tauri::async_runtime::spawn_blocking(move || crate::settings::save(settings))
        .await
        .map_err(|_| "Could not save settings".to_string())?
}

#[tauri::command]
async fn get_desktop_inventory() -> Result<desktop_environment::DesktopInventory, String> {
    tauri::async_runtime::spawn_blocking(desktop_environment::inventory_with_icons)
        .await
        .map_err(|_| "Could not read desktops".to_string())?
}

#[tauri::command]
async fn launch_profile(
    id: String,
    workspace: String,
    desktop: Option<String>,
    retry_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<launch::LaunchResult, String> {
    let service = Arc::clone(&state.service);
    let launches = Arc::clone(&state.launches);
    tauri::async_runtime::spawn_blocking(move || {
        // Held across snapshot, launch and placement, including retries.
        let home = service.vscode_home(&id)?;
        launch::LaunchCoordinator::execute_serialized(
            &launches,
            launch::LaunchRequest {
                profile: &id,
                workspace: &workspace,
                home: &home,
                desktop: desktop.as_deref(),
                retry: retry_token.as_deref(),
            },
            || service.launch_profile(&id, Path::new(&workspace)),
            &launch::NativeDesktopControl,
        )
    })
    .await
    .map_err(|_| "Could not launch the profile".to_string())?
}

#[tauri::command]
async fn launch_standalone_profile(
    id: String,
    workspace: String,
    desktop: Option<String>,
    retry_token: Option<String>,
    state: State<'_, AppState>,
) -> Result<launch::LaunchResult, String> {
    let service = Arc::clone(&state.service);
    let launches = Arc::clone(&state.launches);
    tauri::async_runtime::spawn_blocking(move || {
        let home = service.desktop_home(&id)?;
        launch::LaunchCoordinator::execute_serialized(
            &launches,
            launch::LaunchRequest {
                profile: &id,
                workspace: &workspace,
                home: &home,
                desktop: desktop.as_deref(),
                retry: retry_token.as_deref(),
            },
            || service.launch_standalone_profile(&id, Path::new(&workspace)),
            &launch::StandaloneDesktopControl,
        )
    })
    .await
    .map_err(|_| "Could not launch the Codex app".to_string())?
}

#[tauri::command]
async fn discard_placement(retry_token: String, state: State<'_, AppState>) -> Result<(), String> {
    let launches = Arc::clone(&state.launches);
    tauri::async_runtime::spawn_blocking(move || {
        launches
            .lock()
            .map_err(|_| "Launch coordinator unavailable")?
            .discard(&retry_token);
        Ok(())
    })
    .await
    .map_err(|_| "Could not discard placement".to_string())?
}

#[tauri::command]
async fn delete_profile(id: String, state: State<'_, AppState>) -> Result<(), String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.delete_profile(&id))
        .await
        .map_err(|_| "Could not delete the profile".to_string())?
}

#[tauri::command]
async fn get_runtime_status(
    id: String,
    state: State<'_, AppState>,
) -> Result<ProfileRuntime, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.runtime_status(&id))
        .await
        .map_err(|_| "Could not read profile status".to_string())?
}

#[tauri::command]
async fn get_storage_usage(state: State<'_, AppState>) -> Result<StorageUsage, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.storage_usage())
        .await
        .map_err(|_| "Could not calculate app storage".to_string())?
}

#[tauri::command]
async fn clear_profile_cache(id: String, state: State<'_, AppState>) -> Result<u64, String> {
    let service = Arc::clone(&state.service);
    tauri::async_runtime::spawn_blocking(move || service.clear_profile_cache(&id))
        .await
        .map_err(|_| "Could not clear the profile cache".to_string())?
}

#[tauri::command]
async fn get_launch_environment() -> Result<LaunchEnvironment, String> {
    tauri::async_runtime::spawn_blocking(|| LaunchEnvironment {
        default_workspace: desktop_environment::default_workspace_root()
            .to_string_lossy()
            .into_owned(),
        capabilities: desktop_environment::capabilities(),
    })
    .await
    .map_err(|_| "Could not read launch environment".to_string())
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
async fn get_launch_targets() -> Result<launch_targets::LaunchTargets, String> {
    tauri::async_runtime::spawn_blocking(launch_targets::detect)
        .await
        .map_err(|_| "Could not detect installed applications".to_string())
}

#[tauri::command]
fn get_desktop_integration_status(
    state: State<'_, AppState>,
) -> Result<DesktopIntegrationStatus, String> {
    Ok(state.desktop_integration.status())
}

#[tauri::command]
async fn install_desktop_integration(
    create_desktop_shortcut: bool,
    state: State<'_, AppState>,
) -> Result<DesktopIntegrationStatus, String> {
    let integration = state.desktop_integration.clone();
    tauri::async_runtime::spawn_blocking(move || integration.install(create_desktop_shortcut))
        .await
        .map_err(|_| "Could not install desktop integration".to_string())?
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
            launches: Arc::new(Mutex::new(launch::LaunchCoordinator::default())),
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
            desktop_environment::configure_main_window();
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_desktop_inventory,
            get_executable_settings,
            save_executable_settings,
            discard_placement,
            list_profiles,
            add_profile,
            import_current_profile,
            update_profile,
            validate_auth,
            begin_device_login,
            open_device_login_browser,
            cancel_device_login,
            launch_profile,
            launch_standalone_profile,
            delete_profile,
            get_runtime_status,
            get_storage_usage,
            clear_profile_cache,
            get_launch_environment,
            get_launch_targets,
            list_workspace_directories,
            check_profile_limits,
            get_desktop_integration_status,
            install_desktop_integration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Multi Codex");
}

#[cfg(test)]
mod workspace_picker_tests {
    use super::read_workspace_directories;
    use std::fs;

    #[test]
    fn lists_only_directories_in_sorted_order_and_canonicalizes_location() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("zebra")).unwrap();
        fs::create_dir(root.path().join("Alpha")).unwrap();
        fs::write(root.path().join("notes.txt"), "not a folder").unwrap();

        let listing = read_workspace_directories(root.path().to_str().unwrap()).unwrap();

        assert_eq!(
            listing.path,
            root.path().canonicalize().unwrap().to_string_lossy()
        );
        assert_eq!(
            listing
                .directories
                .iter()
                .map(|entry| entry.name.as_str())
                .collect::<Vec<_>>(),
            ["Alpha", "zebra"]
        );
        assert_eq!(
            listing.parent_path,
            root.path()
                .canonicalize()
                .unwrap()
                .parent()
                .map(|path| path.to_string_lossy().into_owned())
        );
    }

    #[test]
    fn rejects_missing_and_non_directory_paths() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("file.txt");
        fs::write(&file, "hello").unwrap();
        assert!(read_workspace_directories(root.path().join("missing").to_str().unwrap()).is_err());
        assert_eq!(
            read_workspace_directories(file.to_str().unwrap()).unwrap_err(),
            "The selected path is not a folder"
        );
    }

    #[cfg(unix)]
    #[test]
    fn does_not_follow_symlinked_directory_entries() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        fs::create_dir(outside.path().join("secret")).unwrap();
        symlink(outside.path(), root.path().join("linked")).unwrap();

        let listing = read_workspace_directories(root.path().to_str().unwrap()).unwrap();

        assert!(listing.directories.is_empty());
    }
}
