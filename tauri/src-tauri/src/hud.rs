use crate::AppState;
use multi_codex_core::hud as model;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use tauri::{Emitter, Manager, State};

pub struct HudState {
    root: PathBuf,
    pub hidden: Arc<AtomicBool>,
    expanded: AtomicBool,
    settings: Mutex<model::HudSettings>,
    acknowledgments: Mutex<BTreeSet<String>>,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HudSession {
    #[serde(flatten)]
    pub session: model::Session,
    pub event_key: String,
    pub needs_attention: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: String,
    name: String,
    avatar: model::Avatar,
    sessions: Vec<HudSession>,
    limits: Option<serde_json::Value>,
}
#[derive(Serialize)]
pub struct View {
    settings: model::HudSettings,
    accounts: Vec<Account>,
}
#[tauri::command]
pub fn hud_snapshot(app: tauri::AppHandle, state: State<'_, HudState>) -> Result<View, String> {
    let profiles = app.state::<AppState>().service.list_profiles()?;
    let mut snapshots = model::snapshots(&state.root);
    snapshots.sort_by_key(|snapshot| (snapshot.start_time, snapshot.pid));
    let acknowledgments = state
        .acknowledgments
        .lock()
        .map_err(|_| "HUD state unavailable")?;
    let accounts = profiles
        .into_iter()
        .map(|profile| {
            let id = profile.metadata.id;
            let mut sessions = BTreeMap::new();
            let mut limits = None;
            for snapshot in snapshots
                .iter()
                .filter(|snapshot| snapshot.profile_id == id)
            {
                if snapshot.limits.is_some() {
                    limits = snapshot.limits.clone();
                }
                for session in snapshot.sessions.values() {
                    let event_key = format!(
                        "{id}/{}/{}/{}/{}",
                        snapshot.pid, snapshot.start_time, session.id, session.epoch
                    );
                    sessions.insert(
                        session.id.clone(),
                        HudSession {
                            session: session.clone(),
                            needs_attention: session.attention.is_some()
                                && !acknowledgments.contains(&event_key),
                            event_key,
                        },
                    );
                }
            }
            Account {
                avatar: model::avatar(&id),
                id,
                name: profile.metadata.name,
                sessions: sessions.into_values().collect(),
                limits,
            }
        })
        .collect();
    Ok(View {
        settings: state
            .settings
            .lock()
            .map_err(|_| "HUD settings unavailable")?
            .clone(),
        accounts,
    })
}
#[tauri::command]
pub fn hud_save_settings(
    settings: model::HudSettings,
    state: State<'_, HudState>,
) -> Result<(), String> {
    let mut current = state
        .settings
        .lock()
        .map_err(|_| "HUD settings unavailable")?;
    model::save_settings(&state.root, &settings)?;
    *current = settings;
    Ok(())
}
#[tauri::command]
pub fn hud_toggle(state: State<'_, HudState>) {
    state.hidden.fetch_xor(true, Ordering::SeqCst);
}
#[tauri::command]
pub fn hud_set_expanded(expanded: bool, state: State<'_, HudState>) {
    state.expanded.store(expanded, Ordering::SeqCst);
}
#[tauri::command]
pub fn hud_acknowledge(event_key: String, state: State<'_, HudState>) -> Result<(), String> {
    if event_key.len() > 256
        || !event_key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
    {
        return Err("Invalid HUD event identity".into());
    }
    let mut acknowledgments = state
        .acknowledgments
        .lock()
        .map_err(|_| "HUD reminders unavailable")?;
    acknowledgments.insert(event_key);
    // Keep a bounded receipt set; event IDs include the verified process epoch.
    while acknowledgments.len() > 4096 {
        if let Some(first) = acknowledgments.first().cloned() {
            acknowledgments.remove(&first);
        }
    }
    model::atomic_json(
        &state.root.join("hud/acknowledgments.json"),
        &*acknowledgments,
    )
}
#[tauri::command]
pub async fn hud_open_session(
    account_id: String,
    thread_id: Option<String>,
    event_key: Option<String>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let service = Arc::clone(&app.state::<AppState>().service);
    let workspace = thread_id
        .as_ref()
        .and_then(|id| {
            model::snapshots(service.data_directory())
                .into_iter()
                .filter(|s| s.profile_id == account_id)
                .find_map(|s| {
                    s.sessions
                        .get(id)
                        .map(|session| PathBuf::from(&session.cwd))
                })
        })
        .filter(|path| path.is_dir())
        .or_else(dirs::home_dir)
        .ok_or("Home directory unavailable")?;
    tauri::async_runtime::spawn_blocking(move || {
        service.open_desktop_session(&account_id, thread_id.as_deref(), &workspace)
    })
    .await
    .map_err(|_| "Could not open Codex chat")??;
    if let Some(event) = event_key {
        hud_acknowledge(event, app.state::<HudState>())?;
    }
    Ok(())
}
#[tauri::command]
pub fn hud_monitors(app: tauri::AppHandle) -> Result<Vec<String>, String> {
    Ok(app
        .available_monitors()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter_map(|m| m.name().cloned())
        .collect())
}
#[tauri::command]
pub fn hud_drag(app: tauri::AppHandle) -> Result<(), String> {
    app.get_webview_window("hud")
        .ok_or("HUD unavailable")?
        .start_dragging()
        .map_err(|e| e.to_string())
}
pub fn setup(app: &tauri::AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let root = app
        .state::<AppState>()
        .service
        .data_directory()
        .to_path_buf();
    let settings = model::load_settings(&root)?;
    let acknowledgments = std::fs::read(root.join("hud/acknowledgments.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    let hidden = Arc::new(AtomicBool::new(false));
    app.manage(HudState {
        root,
        hidden: hidden.clone(),
        expanded: AtomicBool::new(false),
        settings: Mutex::new(settings),
        acknowledgments: Mutex::new(acknowledgments),
    });
    if !matches!(
        crate::desktop_environment::backend(),
        "macos" | "hyprland" | "gnome" | "kde"
    ) {
        return Ok(());
    }
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "hud",
        tauri::WebviewUrl::App("index.html?view=hud".into()),
    )
    .title("Multi Codex HUD")
    .inner_size(400.0, 88.0)
    .min_inner_size(32.0, 32.0)
    .decorations(false)
    .resizable(true)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .build()?;
    crate::hud_platform::configure(&window);
    crate::hud_platform::shortcut(hidden);
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
        app.plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _, event| {
                    if event.state() == ShortcutState::Pressed {
                        hud_toggle(app.state::<HudState>());
                    }
                })
                .build(),
        )?;
        app.global_shortcut().register("Super+Shift+H")?;
    }
    // Store drag position after a short debounce. Geometry updates below also
    // move the window, so only a changed settled position is written.
    let handle = app.clone();
    std::thread::spawn(move || {
        let mut previous_geometry = String::new();
        let mut previous_position = None;
        loop {
            let state = handle.state::<HudState>();
            let Ok(settings) = state.settings.lock().map(|value| value.clone()) else {
                break;
            };
            let Ok(monitors) = handle.available_monitors() else {
                let _ = window.hide();
                std::thread::sleep(std::time::Duration::from_millis(500));
                continue;
            };
            let monitor = monitors
                .iter()
                .find(|m| m.name() == settings.monitor.as_ref())
                .or_else(|| monitors.first());
            if let Some(monitor) = monitor {
                let scale = monitor.scale_factor();
                let origin = monitor.position().to_logical::<f64>(scale);
                let display = monitor.size().to_logical::<f64>(scale);
                let count = hud_snapshot(handle.clone(), state.clone())
                    .map(|view| {
                        view.accounts
                            .iter()
                            .filter(|a| !settings.only_open || !a.sessions.is_empty())
                            .count()
                    })
                    .unwrap_or(0);
                let base =
                    (count.max(1) as f64 * (settings.size as f64 + 8.0) + 58.0).min(display.width);
                let expanded = state.expanded.load(Ordering::SeqCst);
                let width = if expanded {
                    base.max(340.0).min(display.width)
                } else {
                    base
                };
                let height = if expanded {
                    420.0_f64.min(display.height)
                } else {
                    settings.size as f64 + 32.0
                };
                let x = settings
                    .x
                    .unwrap_or(origin.x + display.width - base - 20.0)
                    .clamp(origin.x, origin.x + (display.width - base).max(0.0));
                let y = settings
                    .y
                    .unwrap_or(origin.y + 24.0)
                    .clamp(origin.y, origin.y + (display.height - height).max(0.0));
                let target_x = (x - (width - base)).max(origin.x);
                let geometry = format!("{width}:{height}:{target_x}:{y}");
                if geometry != previous_geometry {
                    let _ = window.set_size(tauri::LogicalSize::new(width, height));
                    let _ = window.set_position(tauri::LogicalPosition::new(target_x, y));
                    crate::hud_platform::geometry(target_x, y, width, height);
                    previous_geometry = geometry;
                    previous_position = None;
                } else if !expanded {
                    if let Some((position_x, position_y)) =
                        crate::hud_platform::position(&window, scale)
                    {
                        if previous_position == Some((position_x, position_y))
                            && ((position_x - x).abs() > 1.0 || (position_y - y).abs() > 1.0)
                        {
                            if let Ok(mut current) = state.settings.lock() {
                                current.x = Some(position_x);
                                current.y = Some(position_y);
                                current.monitor = monitor.name().cloned();
                                let _ = model::save_settings(&state.root, &current);
                            }
                        }
                        previous_position = Some((position_x, position_y));
                    }
                }
                let visible = settings.enabled
                    && !state.hidden.load(Ordering::SeqCst)
                    && (!settings.only_open || count > 0)
                    && crate::hud_platform::visible(
                        origin.x + display.width / 2.0,
                        origin.y + display.height / 2.0,
                    );
                if visible {
                    if !window.is_visible().unwrap_or(false) {
                        let _ = window.show();
                        previous_geometry.clear();
                    }
                } else if window.is_visible().unwrap_or(false) {
                    let _ = window.hide();
                }
            } else {
                let _ = window.hide();
            }
            let _ = handle.emit_to("hud", "hud-refresh", ());
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    Ok(())
}
