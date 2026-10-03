use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use crate::profiles::Result;

pub const WINDOW_WAIT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopCapabilities {
    pub backend: String,
    pub enumerate_desktops: bool,
    pub enumerate_windows: bool,
    pub move_windows: bool,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopWindow {
    pub id: String,
    pub pid: u32,
    pub application: String,
    pub title: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Desktop {
    pub id: String,
    pub name: String,
    pub monitor: Option<String>,
    pub current: bool,
    pub windows: Vec<DesktopWindow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopInventory {
    pub protocol_version: u32,
    pub capabilities: DesktopCapabilities,
    pub desktops: Vec<Desktop>,
}

impl DesktopInventory {
    pub fn windows(&self) -> Vec<DesktopWindow> {
        let mut seen = HashSet::new();
        self.desktops
            .iter()
            .flat_map(|d| d.windows.iter())
            .filter(|w| seen.insert(w.id.clone()))
            .cloned()
            .collect()
    }

    pub fn require_destination(&self, id: &str) -> Result<()> {
        if !self.capabilities.move_windows {
            return Err(self
                .capabilities
                .reason
                .clone()
                .unwrap_or_else(|| "Desktop placement is unavailable".into()));
        }
        if !self.desktops.iter().any(|d| d.id == id) {
            return Err("This desktop no longer exists. Choose another destination.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct HyprWorkspace {
    id: i32,
    name: String,
    #[serde(default)]
    monitor: String,
}

#[derive(Debug, Deserialize)]
struct HyprClient {
    address: String,
    pid: u32,
    class: String,
    title: String,
    workspace: HyprWorkspace,
    #[serde(default)]
    pinned: bool,
    #[serde(default = "mapped_default")]
    mapped: bool,
}
fn mapped_default() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HyprMonitor {
    name: String,
    active_workspace: HyprWorkspace,
}

pub fn backend() -> &'static str {
    if cfg!(target_os = "macos") {
        return "macos";
    }
    if env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some() {
        return "hyprland";
    }
    let desktop = env::var("XDG_CURRENT_DESKTOP")
        .unwrap_or_default()
        .to_lowercase();
    if desktop.split(':').any(|d| d == "gnome" || d == "ubuntu") {
        "gnome"
    } else if desktop.split(':').any(|d| d == "kde") {
        "kde"
    } else {
        "unsupported"
    }
}

pub fn unsupported_inventory(backend: &str, reason: String) -> DesktopInventory {
    DesktopInventory {
        protocol_version: 1,
        capabilities: DesktopCapabilities {
            backend: backend.into(),
            enumerate_desktops: false,
            enumerate_windows: false,
            move_windows: false,
            reason: Some(reason),
        },
        desktops: Vec::new(),
    }
}

pub fn inventory() -> Result<DesktopInventory> {
    match backend() {
        "hyprland" => hypr_inventory(),
        #[cfg(target_os = "linux")]
        "gnome" | "kde" => crate::linux_desktops::inventory(backend()),
        "macos" => Ok(unsupported_inventory("macos", "Native macOS Spaces enumeration and verified placement have not passed the release feasibility gate. Opening on the current desktop is available.".into())),
        _ => Ok(unsupported_inventory("unsupported", "Desktop control is available on Hyprland, GNOME with the Multi Codex extension, and KDE Plasma 6. Opening on the current desktop is available.".into())),
    }
}

pub fn capabilities() -> DesktopCapabilities {
    inventory()
        .unwrap_or_else(|error| unsupported_inventory(backend(), error))
        .capabilities
}

fn hypr_json<T: serde::de::DeserializeOwned>(kind: &str) -> Result<T> {
    let output = crate::process::output(
        Command::new("hyprctl").args([kind, "-j"]),
        Duration::from_secs(3),
    )?;
    if !output.status.success() {
        return Err("Hyprland workspace controls are unavailable".into());
    }
    serde_json::from_slice(&output.stdout).map_err(|_| format!("Hyprland returned invalid {kind}"))
}

fn hypr_inventory() -> Result<DesktopInventory> {
    let workspaces = hypr_json::<Vec<HyprWorkspace>>("workspaces")?;
    let clients = hypr_json::<Vec<HyprClient>>("clients")?;
    let monitors = hypr_json::<Vec<HyprMonitor>>("monitors")?;
    Ok(group_hyprland(workspaces, clients, monitors))
}

fn group_hyprland(
    mut workspaces: Vec<HyprWorkspace>,
    clients: Vec<HyprClient>,
    monitors: Vec<HyprMonitor>,
) -> DesktopInventory {
    let active: HashMap<i32, String> = monitors
        .into_iter()
        .map(|m| (m.active_workspace.id, m.name))
        .collect();
    workspaces.retain(|w| w.id > 0);
    workspaces.sort_by_key(|w| w.id);
    let desktops = workspaces
        .into_iter()
        .map(|w| Desktop {
            id: format!("hyprland:{}", w.id),
            name: w.name,
            monitor: active
                .get(&w.id)
                .cloned()
                .or_else(|| (!w.monitor.is_empty()).then_some(w.monitor)),
            current: active.contains_key(&w.id),
            windows: clients
                .iter()
                .filter(|c| c.mapped && (c.workspace.id == w.id || c.pinned))
                .map(|c| DesktopWindow {
                    id: c.address.clone(),
                    pid: c.pid,
                    application: c.class.clone(),
                    title: c.title.clone(),
                })
                .collect(),
        })
        .collect();
    DesktopInventory {
        protocol_version: 1,
        capabilities: DesktopCapabilities {
            backend: "hyprland".into(),
            enumerate_desktops: true,
            enumerate_windows: true,
            move_windows: true,
            reason: None,
        },
        desktops,
    }
}

pub fn move_window(id: &str, destination: &str) -> Result<()> {
    inventory()?.require_destination(destination)?;
    match backend() {
        "hyprland" => {
            if !valid_address(id) {
                return Err("Invalid Hyprland window identifier".into());
            }
            if hypr_json::<Vec<HyprClient>>("clients")?.iter().any(|client| client.address == id && client.pinned) {
                return Err("Unpin this window before moving it to one desktop".into());
            }
            let workspace = destination
                .strip_prefix("hyprland:")
                .and_then(|s| s.parse::<i32>().ok())
                .filter(|n| *n > 0)
                .ok_or("Invalid Hyprland desktop identifier")?;
            // Probe without moving anything. Older releases use dispatch; newer releases use Lua.
            let lua_available = crate::process::output(Command::new("hyprctl").args(["eval", "assert(type(hl.get_window)=='function'); assert(type(hl.dsp.window.move)=='function')"]), Duration::from_secs(3)).is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "ok");
            let output = if lua_available {
                crate::process::output(
                    Command::new("hyprctl").args(["eval", &move_window_lua(id, workspace)]),
                    Duration::from_secs(3),
                )?
            } else {
                crate::process::output(
                    Command::new("hyprctl").args([
                        "dispatch",
                        "movetoworkspace",
                        &format!("{workspace},address:{id}"),
                    ]),
                    Duration::from_secs(3),
                )?
            };
            if !output.status.success() || String::from_utf8_lossy(&output.stdout).trim() != "ok" {
                return Err("Hyprland rejected the window operation".into());
            }
        }
        #[cfg(target_os = "linux")]
        "gnome" | "kde" => crate::linux_desktops::move_window(backend(), id, destination)?,
        _ => return Err("Verified desktop placement is unavailable on this platform".into()),
    }
    let deadline = Instant::now() + WINDOW_WAIT;
    while Instant::now() < deadline {
        let snapshot = inventory()?;
        if snapshot
            .desktops
            .iter()
            .any(|d| d.id == destination && d.windows.iter().any(|w| w.id == id))
        {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("VS Code opened, but its destination could not be verified. Retry placement without opening another window.".into())
}

pub fn find_new_window(previous: &HashSet<String>, vscode_home: &Path) -> Result<String> {
    let deadline = Instant::now() + WINDOW_WAIT;
    while Instant::now() < deadline {
        let candidates: Vec<_> = inventory()?
            .windows()
            .into_iter()
            .filter(|c| {
                !previous.contains(&c.id)
                    && crate::profiles::process_uses_profile(c.pid, vscode_home)
            })
            .collect();
        match candidates.as_slice() {
            [window] => return Ok(window.id.clone()),
            [] => {},
            _ => return Err("Multiple new windows match this profile. Close the extra windows and retry placement.".into()),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("VS Code opened, but its new window could not be identified. Retry placement without opening another window.".into())
}

pub fn configure_main_window() {
    if backend() != "hyprland" {
        return;
    }
    let pid = std::process::id();
    thread::spawn(move || {
        let deadline = Instant::now() + WINDOW_WAIT;
        while Instant::now() < deadline {
            if let Ok(clients) = hypr_json::<Vec<HyprClient>>("clients") {
                if let Some(client) = clients.into_iter().find(|client| client.pid == pid) {
                    if valid_address(&client.address) {
                        let _ = crate::process::output(
                            Command::new("hyprctl")
                                .args(["eval", &main_window_lua(&client.address)]),
                            Duration::from_secs(3),
                        );
                    }
                    return;
                }
            }
            thread::sleep(POLL_INTERVAL);
        }
    });
}

pub fn default_workspace_root() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    let preferred = home.join("Desktop").join("code");
    if preferred.is_dir() {
        preferred
    } else {
        home
    }
}

fn valid_address(address: &str) -> bool {
    address
        .strip_prefix("0x")
        .is_some_and(|digits| !digits.is_empty() && digits.chars().all(|c| c.is_ascii_hexdigit()))
}
fn main_window_lua(address: &str) -> String {
    format!("local w=hl.get_window('address:{address}'); assert(w); hl.dispatch(hl.dsp.window.fullscreen({{mode='fullscreen',action='set',window=w}}))")
}
fn move_window_lua(address: &str, workspace: i32) -> String {
    format!("local w=hl.get_window('address:{address}'); assert(w); hl.dispatch(hl.dsp.window.move({{workspace={workspace},follow=true,window=w}}))")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_code_in_window_identifiers() {
        assert!(valid_address("0x12aB90"));
        assert!(!valid_address("0x12'; os.execute('bad')"));
        assert!(!valid_address("12ab"));
    }
    #[test]
    fn groups_windows_keeps_empty_desktops_and_marks_each_monitor() {
        let workspaces = serde_json::from_str(r#"[{"id":2,"name":"Code","monitor":"A"},{"id":12,"name":"Empty","monitor":"B"},{"id":-1,"name":"special"}]"#).unwrap();
        let clients = serde_json::from_str(r#"[{"address":"0xab","pid":42,"class":"Code","title":"Project","workspace":{"id":2,"name":"Code"}}]"#).unwrap();
        let monitors = serde_json::from_str(r#"[{"name":"A","activeWorkspace":{"id":2,"name":"Code"}},{"name":"B","activeWorkspace":{"id":12,"name":"Empty"}}]"#).unwrap();
        let inventory = group_hyprland(workspaces, clients, monitors);
        assert_eq!(inventory.desktops.len(), 2);
        assert_eq!(inventory.desktops[0].windows[0].title, "Project");
        assert!(inventory.desktops[1].windows.is_empty());
        assert!(inventory.desktops.iter().all(|d| d.current));
        assert!(inventory.require_destination("hyprland:12").is_ok());
        assert!(inventory.require_destination("hyprland:7").is_err());
    }
    #[test]
    fn unsupported_placement_is_an_error() {
        assert!(unsupported_inventory("macos", "Not validated".into())
            .require_destination("macos:1")
            .is_err());
    }
}
