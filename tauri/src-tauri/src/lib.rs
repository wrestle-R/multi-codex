mod desktop_integration;
mod profiles;

use desktop_integration::{DesktopIntegration, DesktopIntegrationStatus};
use profiles::{
    default_service, validate_auth_structure, CodexCliRecognizer, KeyringSecretStore,
    ProfileRuntime, ProfileService, ProfileView, SaveProfileInput,
};
use std::sync::Arc;
use tauri::State;

type AppService = ProfileService<KeyringSecretStore, CodexCliRecognizer>;

struct AppState {
    service: Arc<AppService>,
    desktop_integration: DesktopIntegration,
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
    requests_remaining: Option<u32>,
    notes: Option<String>,
    reset_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    state
        .service
        .import_current(name, requests_remaining, notes, reset_date)
}

#[tauri::command]
fn update_profile(
    id: String,
    name: String,
    auth_json: Option<String>,
    requests_remaining: Option<u32>,
    notes: Option<String>,
    reset_date: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProfileView, String> {
    state
        .service
        .update_profile(&id, name, auth_json, requests_remaining, notes, reset_date)
}

#[tauri::command]
fn validate_auth(auth_json: String, state: State<'_, AppState>) -> Result<String, String> {
    validate_auth_structure(&auth_json)?;
    profiles::AuthRecognizer::recognize(&CodexCliRecognizer, &auth_json)?;
    let _ = state;
    validate_auth_structure(&auth_json)
}

#[tauri::command]
fn launch_profile(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.service.launch_profile(&id)
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
            launch_profile,
            delete_profile,
            get_runtime_status,
            get_desktop_integration_status,
            install_desktop_integration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Multi Codex");
}
