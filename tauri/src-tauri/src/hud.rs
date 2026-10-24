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
    panel_above: AtomicBool,
    platform_error: Mutex<Option<String>>,
    drag: Mutex<Option<LayerDrag>>,
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
    pub client: String,
    pub process_id: u32,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    id: String,
    name: String,
    avatar: model::Avatar,
    sessions: Vec<HudSession>,
    monitoring_message: Option<&'static str>,
    limits: Option<serde_json::Value>,
}
#[derive(Serialize)]
pub struct View {
    settings: model::HudSettings,
    accounts: Vec<Account>,
    #[serde(rename = "panelAbove")]
    panel_above: bool,
    #[serde(rename = "platformError")]
    platform_error: Option<String>,
}
#[tauri::command]
pub fn hud_snapshot(app: tauri::AppHandle, state: State<'_, HudState>) -> Result<View, String> {
    let profiles = app.state::<AppState>().service.list_profiles()?;
    let mut snapshots = model::live_snapshots(&state.root);
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
                        (snapshot.pid, session.id.clone()),
                        HudSession {
                            session: session.clone(),
                            needs_attention: session.attention.is_some()
                                && !acknowledgments.contains(&event_key),
                            event_key,
                            client: snapshot.client.clone(),
                            process_id: snapshot.pid,
                        },
                    );
                }
            }
            let monitoring_message = snapshots.iter().any(|s| s.profile_id == id && s.client == "desktop")
                .then(|| multi_codex_core::hud_desktop::visible(&state.root, &id)).filter(Option::is_none)
                .map(|_| "Open this account normally again when convenient to connect Desktop visibility. Cached chats are excluded.");
            Account {
                monitoring_message,
                avatar: model::avatar(&id),
                id,
                name: profile.metadata.name,
                sessions: sessions.into_values().collect(),
                limits,
            }
        })
        .collect();
    Ok(View {
        platform_error: state
            .platform_error
            .lock()
            .ok()
            .and_then(|error| error.clone()),
        panel_above: state.panel_above.load(Ordering::SeqCst),
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
    // Saving preferences is also an explicit way back from a hidden Orbit.
    state.hidden.store(false, Ordering::SeqCst);
    Ok(())
}
/// Change only the enabled flag, preserving concurrent display/position edits.
#[tauri::command]
pub fn hud_set_enabled(enabled: bool, state: State<'_, HudState>) -> Result<(), String> {
    let mut current = state
        .settings
        .lock()
        .map_err(|_| "Orbit settings unavailable")?;
    let mut next = current.clone();
    next.enabled = enabled;
    model::save_settings(&state.root, &next)?;
    *current = next;
    if enabled {
        state.hidden.store(false, Ordering::SeqCst);
    }
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
pub fn hud_set_input_regions(
    rectangles: Vec<crate::hud_layer::InputRectangle>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let window = app.get_webview_window("hud").ok_or("Orbit unavailable")?;
    crate::hud_layer::input(&window, rectangles)
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
    client: Option<String>,
    process_id: Option<u32>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let service = Arc::clone(&app.state::<AppState>().service);
    if client.as_deref().is_some_and(|c| c != "desktop") {
        let thread = thread_id.as_ref().ok_or("Chat identity unavailable")?;
        let pid = process_id.ok_or("Chat process unavailable")?;
        multi_codex_core::hud_sessions::focus(service.data_directory(), &account_id, thread, pid)?;
        if let Some(event) = event_key {
            hud_acknowledge(event, app.state::<HudState>())?;
        }
        return Ok(());
    }
    let workspace = thread_id
        .as_ref()
        .and_then(|id| {
            model::live_snapshots(service.data_directory())
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
pub fn hud_drag(
    phase: Option<String>,
    app: tauri::AppHandle,
    state: State<'_, HudState>,
) -> Result<bool, String> {
    let window = app.get_webview_window("hud").ok_or("Orbit unavailable")?;
    if !crate::hud_layer::active() {
        window.start_dragging().map_err(|e| e.to_string())?;
        return Ok(false);
    }
    let phase = phase.as_deref().unwrap_or("start");
    if !matches!(phase, "start" | "update" | "end") {
        return Err("Invalid Orbit drag".into());
    }
    let cursor = crate::hud_platform::cursor_position().ok_or("Pointer position unavailable")?;
    let mut drag = state.drag.lock().map_err(|_| "Orbit drag unavailable")?;
    let mut settings = state
        .settings
        .lock()
        .map_err(|_| "Orbit settings unavailable")?;
    if phase == "start" {
        let (x, mut y, width, height) =
            crate::hud_layer::rectangle().ok_or("Orbit position unavailable")?;
        if state.panel_above.load(Ordering::SeqCst) {
            y += height - (settings.size as f64 + 32.0);
        }
        let monitors = app.available_monitors().map_err(|e| e.to_string())?;
        let monitor = monitors
            .iter()
            .find(|m| m.name() == settings.monitor.as_ref())
            .or_else(|| monitors.first())
            .ok_or("Display unavailable")?;
        let origin = monitor.position().to_logical::<f64>(monitor.scale_factor());
        let display = monitor.size().to_logical::<f64>(monitor.scale_factor());
        *drag = Some(LayerDrag {
            cursor,
            origin: (x, y),
            width,
            bounds: (origin.x, origin.y, display.width, display.height),
        });
        state.expanded.store(false, Ordering::SeqCst);
    }
    if let Some(current) = drag.as_ref() {
        let (mx, my, mw, mh) = current.bounds;
        let height = settings.size as f64 + 32.0;
        let x = (current.origin.0 + cursor.0 - current.cursor.0)
            .clamp(mx, mx + (mw - current.width).max(0.0));
        let y =
            (current.origin.1 + cursor.1 - current.cursor.1).clamp(my, my + (mh - height).max(0.0));
        settings.x = Some(x);
        settings.y = Some(y);
        crate::hud_layer::geometry(&window, x, y, current.width, height);
    }
    if phase == "end" {
        *drag = None;
        model::save_settings(&state.root, &settings)?;
    }
    Ok(true)
}

struct LayerDrag {
    cursor: (f64, f64),
    origin: (f64, f64),
    width: f64,
    bounds: (f64, f64, f64, f64),
}

#[derive(Debug)]
struct OrbitGeometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    panel_above: bool,
}

fn orbit_geometry(
    settings: &model::HudSettings,
    count: usize,
    ox: f64,
    oy: f64,
    dw: f64,
    dh: f64,
    expanded: bool,
) -> OrbitGeometry {
    // The width never changes on hover. Too many accounts scroll at their
    // chosen size instead of shrinking. Expansion keeps the strip stationary,
    // including at the bottom edge where the panel opens upwards.
    let width = (count.max(1) as f64 * (settings.size as f64 + 8.0) + 74.0)
        .clamp(340.0, 960.0)
        .min(dw);
    let strip_height = (settings.size as f64 + 32.0).min(dh);
    let x = settings
        .x
        .unwrap_or(ox + dw - width - 20.0)
        .clamp(ox, ox + (dw - width).max(0.0));
    let strip_y = settings
        .y
        .unwrap_or(oy + 24.0)
        .clamp(oy, oy + (dh - strip_height).max(0.0));
    let below = (oy + dh - strip_y - strip_height).max(0.0);
    let above = (strip_y - oy).max(0.0);
    let panel_above = below < 310.0 && above > below;
    let extra = if expanded {
        310.0_f64.min(if panel_above { above } else { below })
    } else {
        0.0
    };
    OrbitGeometry {
        x,
        y: strip_y - if panel_above { extra } else { 0.0 },
        width,
        height: strip_height + extra,
        panel_above,
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    #[test]
    fn hover_keeps_the_strip_still_at_every_display_edge() {
        for y in [0.0, 24.0, 550.0, 1128.0] {
            let settings = model::HudSettings {
                x: Some(0.0),
                y: Some(y),
                size: 40,
                ..Default::default()
            };
            let small = orbit_geometry(&settings, 7, 0.0, 0.0, 1920.0, 1200.0, false);
            let large = orbit_geometry(&settings, 7, 0.0, 0.0, 1920.0, 1200.0, true);
            assert_eq!(small.width, large.width);
            assert_eq!(small.x, large.x);
            let strip_y = large.y
                + if large.panel_above {
                    large.height - small.height
                } else {
                    0.0
                };
            assert_eq!(strip_y, small.y);
            assert!(large.y >= 0.0 && large.y + large.height <= 1200.0);
        }
    }
    #[test]
    fn bounds_fit_small_scaled_and_negative_origin_displays() {
        let settings = model::HudSettings {
            x: Some(-3000.0),
            y: Some(2000.0),
            size: 100,
            ..Default::default()
        };
        let g = orbit_geometry(&settings, 25, -1280.0, -200.0, 280.0, 240.0, true);
        assert_eq!(g.width, 280.0);
        assert!(g.x >= -1280.0 && g.x + g.width <= -1000.0);
        assert!(g.y >= -200.0 && g.y + g.height <= 40.0);
    }
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
        panel_above: AtomicBool::new(false),
        platform_error: Mutex::new(None),
        drag: Mutex::new(None),
        settings: Mutex::new(settings),
        acknowledgments: Mutex::new(acknowledgments),
    });
    if !matches!(
        crate::desktop_environment::backend(),
        "macos" | "hyprland" | "gnome" | "kde"
    ) {
        return Ok(());
    }
    let constrained_gtk = cfg!(target_os = "linux")
        && matches!(crate::desktop_environment::backend(), "gnome" | "kde");
    let mut builder = tauri::WebviewWindowBuilder::new(
        app,
        "hud",
        tauri::WebviewUrl::App("index.html?view=hud".into()),
    )
    .title("Multi Codex Orbit")
    .inner_size(400.0, 88.0)
    .min_inner_size(32.0, 32.0)
    .decorations(false)
    // GTK's non-resizable mode requests WebKit's natural 200px height.
    // Equal minimum/maximum bounds constrain resizing without that fallback.
    .resizable(constrained_gtk)
    .transparent(true)
    .shadow(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false);
    if constrained_gtk {
        builder = builder
            .min_inner_size(400.0, 88.0)
            .max_inner_size(400.0, 88.0);
    }
    let window = builder.build()?;
    if let Err(error) = crate::hud_layer::setup(&window) {
        if let Ok(mut message) = app.state::<HudState>().platform_error.lock() {
            *message = Some(error);
        }
        let _ = window.close();
        return Ok(());
    }
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
                crate::hud_layer::show(&window, false);
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
                let expanded = state.expanded.load(Ordering::SeqCst);
                let layout = orbit_geometry(
                    &settings,
                    count,
                    origin.x,
                    origin.y,
                    display.width,
                    display.height,
                    expanded,
                );
                let OrbitGeometry {
                    x,
                    y,
                    width,
                    height,
                    panel_above,
                    ..
                } = layout;
                state.panel_above.store(panel_above, Ordering::SeqCst);
                let geometry = format!("{width}:{height}:{x}:{y}");
                // Some GTK/Wayland transitions omit DOM mouseleave. Native
                // pointer truth closes hover without relying on keyboard focus.
                if expanded && crate::hud_platform::pointer_outside(x, y, width, height) {
                    let _ = handle.emit_to("hud", "hud-pointer-left", ());
                }
                let resized = crate::hud_platform::size(&window, scale)
                    .is_some_and(|(w, h)| (w - width).abs() > 1.0 || (h - height).abs() > 1.0);
                if geometry != previous_geometry || resized {
                    // GTK and the compositor must agree on the same bounds. User
                    // resizing cannot compress slots or stretch the hover panel.
                    if crate::hud_layer::active() {
                        crate::hud_layer::geometry(&window, x, y, width, height);
                    } else {
                        let _ = window.set_size_constraints(tauri::WindowSizeConstraints {
                            min_width: Some(tauri::LogicalUnit::new(width).into()),
                            min_height: Some(tauri::LogicalUnit::new(height).into()),
                            max_width: Some(tauri::LogicalUnit::new(width).into()),
                            max_height: Some(tauri::LogicalUnit::new(height).into()),
                        });
                        let _ = window.set_size(tauri::LogicalSize::new(width, height));
                        let _ = window.set_position(tauri::LogicalPosition::new(x, y));
                        crate::hud_platform::geometry(x, y, width, height);
                    }
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
                    if !crate::hud_layer::is_visible(&window) {
                        crate::hud_layer::show(&window, true);
                        previous_geometry.clear();
                    }
                } else if crate::hud_layer::is_visible(&window) {
                    crate::hud_layer::show(&window, false);
                }
            } else {
                crate::hud_layer::show(&window, false);
            }
            let _ = handle.emit_to("hud", "hud-refresh", ());
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
    });
    Ok(())
}
