//! The macOS account shortcuts run in an accessory process with a single popover.
use crate::{default_service, AppService};
use base64::{engine::general_purpose::STANDARD, Engine};
use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Duration;
use tauri::image::Image;
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindowBuilder,
};
use tauri_plugin_notification::NotificationExt;

pub const ARGUMENT: &str = "--menu-bar-avatars";
const TRAY: &str = "menu-bar-mascot";
const POPOVER: &str = "menu-bar";
const ACTION: &str = "--menu-bar-action";
const CACHE: &str = "avatars.json";
type Icons = BTreeMap<String, String>;

#[derive(Default)]
pub struct PendingActions(Mutex<Vec<String>>);

fn cache_dir(app: &AppHandle) -> tauri::Result<PathBuf> {
    let name = if cfg!(debug_assertions) {
        "menu-bar-avatars-dev"
    } else {
        "menu-bar-avatars"
    };
    Ok(app.path().app_cache_dir()?.join(name))
}

pub fn start() -> io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .arg(ARGUMENT)
        .spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

#[tauri::command]
pub fn save_menu_bar_avatars(
    app: AppHandle,
    window: tauri::WebviewWindow,
    icons: Icons,
) -> Result<(), String> {
    if window.label() != "main" {
        return Err("Only the launcher can publish menu bar avatars".into());
    }
    let dir = cache_dir(&app).map_err(|e| e.to_string())?;
    let content = serde_json::to_vec(&icons).map_err(|e| e.to_string())?;
    let path = dir.join(CACHE);
    if fs::read(&path).ok().as_deref() != Some(&content) {
        fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut file = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
        file.write_all(&content).map_err(|e| e.to_string())?;
        file.persist(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn open(app: &AppHandle, id: &str) {
    let _ = app.state::<mpsc::Sender<String>>().send(id.to_string());
}

fn launch_accounts(app: AppHandle, service: Arc<AppService>, clicks: mpsc::Receiver<String>) {
    for id in clicks {
        let result = dirs::home_dir()
            .ok_or_else(|| "Home directory unavailable".to_string())
            .and_then(|home| service.open_desktop_session(&id, None, &home));
        if let Err(error) = result {
            eprintln!("Menu bar could not open Codex: {error}");
            let _ = app
                .notification()
                .builder()
                .title("Could not open Codex")
                .body(error)
                .show();
        }
    }
}

#[derive(serde::Serialize)]
struct Account {
    id: String,
    name: String,
    icon: Option<String>,
}

fn read_icons(app: &AppHandle) -> Icons {
    cache_dir(app)
        .ok()
        .and_then(|dir| fs::read(dir.join(CACHE)).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

#[derive(serde::Serialize)]
struct MenuContents {
    accounts: Vec<Account>,
    codex_available: bool,
    targets: Vec<(&'static str, &'static str)>,
}

#[tauri::command]
fn menu_bar_accounts(app: AppHandle, window: tauri::WebviewWindow) -> Result<MenuContents, String> {
    if window.label() != POPOVER {
        return Err("Menu bar window required".into());
    }
    let mut icons = read_icons(&app);
    let mut accounts: Vec<_> = app
        .state::<Arc<AppService>>()
        .list_profile_metadata()?
        .into_iter()
        .map(|profile| Account {
            icon: icons.remove(&profile.id),
            id: profile.id,
            name: profile.name,
        })
        .collect();
    accounts.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then(a.id.cmp(&b.id))
    });
    let installed = crate::launch_targets::detect();
    let mut targets = crate::account_menu::launch_actions(&installed);
    targets.retain(|(target, _)| *target != "standalone");
    Ok(MenuContents {
        accounts,
        codex_available: installed.standalone_installed,
        targets,
    })
}

pub fn launcher_action(mut args: impl Iterator<Item = String>) -> Option<String> {
    (args.next()?.as_str() == ACTION)
        .then(|| args.next())
        .flatten()
}

pub fn dispatch(app: &AppHandle, action: String) {
    if action == "quit" {
        app.exit(0);
        return;
    }
    if action == "open" {
        return;
    }
    if action == "hud-toggle" {
        if let Some(state) = app.try_state::<crate::hud::HudState>() {
            crate::hud::hud_toggle(state);
            return;
        }
    }
    if let Some(pending) = app.try_state::<PendingActions>() {
        if let Ok(mut queue) = pending.0.lock() {
            queue.push(action);
        }
        let _ = app.emit_to("main", "menu-bar-action", ());
    }
}

#[tauri::command]
pub fn take_menu_bar_actions(
    window: tauri::WebviewWindow,
    pending: State<'_, PendingActions>,
) -> Result<Vec<String>, String> {
    if window.label() != "main" {
        return Err("Launcher window required".into());
    }
    let mut queue = pending.0.lock().map_err(|e| e.to_string())?;
    Ok(std::mem::take(&mut *queue))
}

#[tauri::command]
fn menu_bar_action(
    app: AppHandle,
    window: tauri::WebviewWindow,
    action: &str,
    id: Option<String>,
    target: Option<String>,
) -> Result<(), String> {
    if window.label() != POPOVER {
        return Err("Menu bar window required".into());
    }
    let launcher_action = match action {
        "dismiss" => None,
        "launch" => {
            open(&app, &id.ok_or("Account required")?);
            None
        }
        "launch-target" => {
            let target = target.ok_or("Launch target required")?;
            if !matches!(target.as_str(), "vscode" | "t3" | "cli") {
                return Err("Unknown launch target".into());
            }
            Some(format!("launch:{target}:{}", id.ok_or("Account required")?))
        }
        "open" | "add" | "hud-toggle" | "hud-settings" | "refresh" | "quit" => {
            Some(action.to_owned())
        }
        _ => return Err("Unknown menu bar action".into()),
    };
    if let Some(action) = launcher_action {
        send_launcher_action(&app, action).map_err(|e| e.to_string())?;
    }
    window.hide().map_err(|e| e.to_string())
}

fn send_launcher_action(app: &AppHandle, action: String) -> io::Result<()> {
    let mut child = Command::new(std::env::current_exe()?)
        .args([ACTION, &action])
        .spawn()?;
    let app = app.clone();
    std::thread::spawn(move || {
        if child.wait().is_ok_and(|status| status.success()) && action == "quit" {
            app.exit(0);
        }
    });
    Ok(())
}

fn popover_height(app: &AppHandle, scale: f64, extra_rows: usize) -> u32 {
    let count = app
        .state::<Arc<AppService>>()
        .list_profile_metadata()
        .map_or(0, |profiles| profiles.len());
    ((235 + count.clamp(1, 6) * 46 + extra_rows.min(3) * 30 + usize::from(extra_rows > 0) * 5)
        as f64
        * scale)
        .round() as u32
}

#[tauri::command]
fn menu_bar_expand(
    app: AppHandle,
    window: tauri::WebviewWindow,
    rows: usize,
) -> Result<(), String> {
    if window.label() != POPOVER {
        return Err("Menu bar window required".into());
    }
    let scale = window.scale_factor().map_err(|e| e.to_string())?;
    let width = window.inner_size().map_err(|e| e.to_string())?.width;
    window
        .set_size(PhysicalSize::new(width, popover_height(&app, scale, rows)))
        .map_err(|e| e.to_string())
}

fn toggle_popover(app: &AppHandle, anchor: tauri::Rect) -> tauri::Result<()> {
    let window = if let Some(window) = app.get_webview_window(POPOVER) {
        if window.is_visible()? {
            return window.hide();
        }
        window
    } else {
        WebviewWindowBuilder::new(
            app,
            POPOVER,
            WebviewUrl::App("index.html?view=menu-bar".into()),
        )
        .title("Multi Codex Menu")
        .visible(false)
        .decorations(false)
        .always_on_top(true)
        .resizable(false)
        .skip_taskbar(true)
        .transparent(true)
        .build()?
    };
    let point = anchor.position.to_physical::<i32>(1.0);
    let monitor = window
        .available_monitors()?
        .into_iter()
        .find(|monitor| {
            let origin = monitor.position();
            let size = monitor.size();
            point.x >= origin.x
                && point.x < origin.x + size.width as i32
                && point.y >= origin.y
                && point.y < origin.y + size.height as i32
        })
        .or(window.current_monitor()?);
    let scale = monitor
        .as_ref()
        .map_or(1.0, |monitor| monitor.scale_factor());
    let point = anchor.position.to_physical::<i32>(scale);
    let anchor_size = anchor.size.to_physical::<u32>(scale);
    let width = (312.0 * scale).round() as u32;
    let height = popover_height(app, scale, 0);
    let mut x = point.x + (anchor_size.width as i32 - width as i32) / 2;
    let y = point.y + anchor_size.height as i32 + 4;
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let left = area.position.x + 6;
        let right = area.position.x + area.size.width as i32 - width as i32 - 6;
        x = x.clamp(left, right.max(left));
    }
    window.set_size(PhysicalSize::new(width, height))?;
    window.set_position(PhysicalPosition::new(x, y))?;
    let _ = app.emit_to(POPOVER, "menu-bar-open", ());
    window.show()?;
    window.set_focus()
}

fn watch(app: AppHandle, service: Arc<AppService>) {
    let mut previous = None;
    let mut report_geometry = std::env::var_os("MULTI_CODEX_MENU_PROBE").is_some();
    loop {
        if report_geometry {
            if let Some(tray) = app.tray_by_id(TRAY) {
                if let Ok(Some(rect)) = tray.rect() {
                    let scale = app
                        .primary_monitor()
                        .ok()
                        .flatten()
                        .map_or(1.0, |monitor| monitor.scale_factor());
                    let point = rect.position.to_logical::<f64>(scale);
                    let size = rect.size.to_logical::<f64>(scale);
                    if size.width > 0.0 && size.height > 0.0 {
                        println!(
                            "Multi Codex mascot location: {} {} {}",
                            std::process::id(),
                            point.x + size.width / 2.0,
                            point.y + size.height / 2.0
                        );
                        report_geometry = false;
                    }
                }
            }
        }
        let icons = read_icons(&app);
        let mascot = service.list_profile_metadata().ok().and_then(|profiles| {
            profiles
                .iter()
                .filter(|profile| icons.contains_key(&profile.id))
                .min_by_key(|profile| &profile.id)
                .and_then(|profile| icons.get(&profile.id))
                .cloned()
        });
        if mascot != previous {
            if let Some(tray) = app.tray_by_id(TRAY) {
                let icon = mascot
                    .as_deref()
                    .and_then(|icon| STANDARD.decode(icon).ok())
                    .and_then(|png| Image::from_bytes(&png).ok());
                let template = icon.is_none();
                let icon = icon
                    .or_else(|| Image::from_bytes(include_bytes!("../icons/menu-bar.png")).ok());
                let _ = tray.set_icon_with_as_template(icon, template);
            }
            previous = mascot;
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

pub fn run(mut context: tauri::Context<tauri::Wry>) -> tauri::Result<()> {
    context.config_mut().app.windows.clear();
    let app = tauri::Builder::default()
        .enable_macos_default_menu(false)
        .plugin(tauri_plugin_notification::init())
        .invoke_handler(tauri::generate_handler![
            menu_bar_accounts,
            menu_bar_action,
            menu_bar_expand
        ])
        .on_window_event(|window, event| {
            if window.label() == POPOVER && matches!(event, tauri::WindowEvent::Focused(false)) {
                let _ = window.hide();
            }
        })
        .on_tray_icon_event(|app, event| {
            if let TrayIconEvent::Click {
                id,
                rect,
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                if id.as_ref() == TRAY {
                    if let Err(error) = toggle_popover(app, rect) {
                        eprintln!("Could not open the menu bar dropdown: {error}");
                    }
                }
            }
        })
        .setup(|app| {
            let dir = cache_dir(app.handle())?;
            fs::create_dir_all(&dir)?;
            let lock = OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(dir.join("helper.lock"))?;
            if let Err(error) = FileExt::try_lock_exclusive(&lock) {
                if error.kind() != io::ErrorKind::WouldBlock {
                    return Err(error.into());
                }
                app.handle().exit(0);
                return Ok(());
            }
            let service = Arc::new(default_service().map_err(io::Error::other)?);
            let (clicks, requests) = mpsc::channel();
            app.manage(clicks);
            app.manage(Arc::clone(&service));
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            TrayIconBuilder::with_id(TRAY)
                .icon(Image::from_bytes(include_bytes!("../icons/menu-bar.png"))?)
                .icon_as_template(true)
                .tooltip("Multi Codex")
                .build(app)?;
            let handle = app.handle().clone();
            let launch_service = Arc::clone(&service);
            std::thread::spawn(move || launch_accounts(handle, launch_service, requests));
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                let _lock = lock;
                watch(handle, service);
            });
            Ok(())
        })
        .build(context)?;
    app.run(|app, event| {
        if matches!(event, tauri::RunEvent::Reopen { .. }) {
            let _ = send_launcher_action(app, "open".into());
        }
    });
    Ok(())
}
