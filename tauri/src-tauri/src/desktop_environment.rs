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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
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
    pub fn current_destination(&self, launcher_pid: u32) -> Option<&str> {
        let mut current = self.desktops.iter().filter(|desktop| desktop.current);
        if let Some(desktop) = current.clone().find(|desktop| {
            desktop
                .windows
                .iter()
                .any(|window| window.pid == launcher_pid)
        }) {
            return Some(&desktop.id);
        }
        let only = current.next()?;
        current.next().is_none().then_some(only.id.as_str())
    }
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
    #[serde(default, rename = "stableId")]
    stable_id: Option<String>,
    pid: u32,
    class: String,
    title: String,
    workspace: HyprWorkspace,
    #[serde(default)]
    pinned: bool,
    #[serde(default)]
    floating: bool,
    #[serde(default)]
    fullscreen: u8,
    #[serde(default = "mapped_default")]
    mapped: bool,
}
impl HyprClient {
    fn window_id(&self) -> String {
        self.stable_id
            .as_ref()
            .map(|id| format!("hyprland-window:{id}"))
            .unwrap_or_else(|| self.address.clone())
    }
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
    if cfg!(target_os = "windows") {
        return "windows";
    }
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
        "windows" => Ok(unsupported_inventory("windows", "Windows virtual desktop placement is unavailable. Apps open on the current desktop.".into())),
        "macos" => Ok(unsupported_inventory("macos", "Native macOS Spaces enumeration and verified placement have not passed the release feasibility gate. Opening on the current desktop is available.".into())),
        _ => Ok(unsupported_inventory("unsupported", "Desktop control is available on Hyprland, GNOME with the Multi Codex extension, and KDE Plasma 6. Opening on the current desktop is available.".into())),
    }
}

pub fn inventory_with_icons() -> Result<DesktopInventory> {
    let snapshot = inventory()?;
    #[cfg(target_os = "linux")]
    {
        let mut snapshot = snapshot;
        crate::application_icons::decorate(&mut snapshot);
        Ok(snapshot)
    }
    #[cfg(not(target_os = "linux"))]
    Ok(snapshot)
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
    // Hyprland creates workspaces lazily. These are valid compositor destinations
    // even before a window has caused the workspace to appear in `workspaces`.
    for id in 1..=10 {
        if !workspaces.iter().any(|workspace| workspace.id == id) {
            workspaces.push(HyprWorkspace {
                id,
                name: id.to_string(),
                monitor: String::new(),
            });
        }
    }
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
            name: if w.name == w.id.to_string() {
                format!("Desktop {}", w.id)
            } else {
                w.name
            },
            monitor: active
                .get(&w.id)
                .cloned()
                .or_else(|| (!w.monitor.is_empty()).then_some(w.monitor)),
            current: active.contains_key(&w.id),
            windows: clients
                .iter()
                .filter(|c| c.mapped && (c.workspace.id == w.id || c.pinned))
                .map(|c| DesktopWindow {
                    id: c.window_id(),
                    pid: c.pid,
                    application: c.class.clone(),
                    title: c.title.clone(),
                    icon: None,
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
            let client = hypr_json::<Vec<HyprClient>>("clients")?
                .into_iter()
                .find(|client| client.window_id() == id)
                .ok_or("The selected window no longer exists")?;
            if !valid_address(&client.address) {
                return Err("Invalid Hyprland window identifier".into());
            }
            if client.pinned {
                return Err("Unpin this window before moving it to one desktop".into());
            }
            let address = &client.address;
            let workspace = destination
                .strip_prefix("hyprland:")
                .and_then(|s| s.parse::<i32>().ok())
                .filter(|n| *n > 0)
                .ok_or("Invalid Hyprland desktop identifier")?;
            // Probe without moving anything. Older releases use dispatch; newer releases use Lua.
            let lua_available = crate::process::output(Command::new("hyprctl").args(["eval", "assert(type(hl.get_window)=='function'); assert(type(hl.dsp.window.move)=='function')"]), Duration::from_secs(3)).is_ok_and(|o| o.status.success() && String::from_utf8_lossy(&o.stdout).trim() == "ok");
            let output = if lua_available {
                crate::process::output(
                    Command::new("hyprctl").args(["eval", &move_window_lua(address, workspace)]),
                    Duration::from_secs(3),
                )?
            } else {
                crate::process::output(
                    Command::new("hyprctl").args([
                        "dispatch",
                        "movetoworkspace",
                        &format!("{workspace},address:{address}"),
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
        snapshot.require_destination(destination)?;
        if snapshot
            .desktops
            .iter()
            .any(|d| d.id == destination && d.windows.iter().any(|w| w.id == id))
        {
            return Ok(());
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("The app opened, but its destination could not be verified. Retry placement without opening another window.".into())
}

pub fn find_new_window(previous: &HashSet<String>, vscode_home: &Path) -> Result<String> {
    find_profile_window(previous, vscode_home, false)
}

pub(crate) fn find_profile_window(
    previous: &HashSet<String>,
    profile_home: &Path,
    reuse_existing: bool,
) -> Result<String> {
    let deadline = Instant::now() + WINDOW_WAIT;
    while Instant::now() < deadline {
        let snapshot = inventory()?;
        if !snapshot.capabilities.enumerate_windows {
            return Err(snapshot
                .capabilities
                .reason
                .unwrap_or_else(|| "Window enumeration is unavailable".into()));
        }
        let candidates: Vec<_> = snapshot
            .windows()
            .into_iter()
            .filter(|c| {
                (reuse_existing || !previous.contains(&c.id))
                    && crate::profiles::window_process_uses_profile(c.pid, profile_home)
            })
            .collect();
        match candidates.as_slice() {
            [window] => return Ok(window.id.clone()),
            [] => {}
            _ => return Err(
                "Multiple windows match this profile. Close the extra windows and retry placement."
                    .into(),
            ),
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err("The app opened, but its window could not be identified. Retry placement without opening another window.".into())
}

pub fn configure_main_window() {
    if backend() != "hyprland" {
        return;
    }
    static MONITORING: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if MONITORING.swap(true, std::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let pid = std::process::id();
    thread::spawn(move || loop {
        if let Ok(clients) = hypr_json::<Vec<HyprClient>>("clients") {
            if let Some(client) = clients
                .into_iter()
                .find(|client| client.pid == pid && client.mapped)
            {
                // GTK/compositor focus transitions can restore a floating window
                // long after startup. Keep its tiled, maximized state throughout
                // launches and desktop changes without focusing or moving it.
                if (client.floating || client.fullscreen != 1) && valid_address(&client.address) {
                    let _ = crate::process::output(
                        Command::new("hyprctl").args(["eval", &main_window_lua(&client.address)]),
                        Duration::from_secs(3),
                    );
                }
            }
        }
        thread::sleep(Duration::from_millis(250));
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
    // A floating maximize leaves a popup-sized restore state when another app
    // takes the workspace. Keep the launcher tiled underneath its maximize.
    format!(
        "local w=hl.get_window('address:{address}'); assert(w); \
         hl.dispatch(hl.dsp.window.fullscreen_state({{internal=0,client=0,window=w}})); \
         hl.dispatch(hl.dsp.window.float({{action='unset',window=w}})); \
         hl.dispatch(hl.dsp.window.fullscreen_state({{internal=1,client=1,window=w}}))"
    )
}
fn move_window_lua(address: &str, workspace: i32) -> String {
    format!("local w=hl.get_window('address:{address}'); assert(w); hl.dispatch(hl.dsp.window.move({{workspace={workspace},follow=true,window=w}}))")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires a live Hyprland session; reads inventory only"]
    fn live_hyprland_inventory() {
        let inventory = inventory_with_icons().unwrap();
        assert!(inventory.capabilities.enumerate_desktops);
        for id in 1..=10 {
            assert!(inventory
                .require_destination(&format!("hyprland:{id}"))
                .is_ok());
        }
        assert!(inventory.desktops.iter().any(|d| d.current));
        if let Ok(home) = env::var("MULTI_CODEX_TEST_EXISTING_HOME") {
            assert!(inventory.windows().iter().any(|window| {
                crate::profiles::window_process_uses_profile(window.pid, Path::new(&home))
            }));
        }
        assert!(inventory
            .windows()
            .iter()
            .filter(|window| matches!(
                window.application.as_str(),
                "Visual Studio Code" | "Zen Browser" | "Multi Codex"
            ))
            .all(|window| window.icon.is_some()));
        println!(
            "Verified live Hyprland inventory: {} desktops, {} windows",
            inventory.desktops.len(),
            inventory.windows().len()
        );
    }

    #[test]
    #[cfg(target_os = "linux")]
    #[ignore = "opens disposable VS Code windows in a live Hyprland session"]
    fn live_hyprland_vscode_placement() {
        assert_eq!(
            env::var("MULTI_CODEX_TEST_HYPR_PLACEMENT").as_deref(),
            Ok("1")
        );
        let before = hypr_inventory().unwrap();
        let previous: HashSet<_> = before
            .windows()
            .into_iter()
            .map(|window| window.id)
            .collect();
        let clients = hypr_json::<Vec<HyprClient>>("clients").unwrap();
        let executable = clients
            .iter()
            .filter(|client| {
                matches!(
                    client.class.to_ascii_lowercase().as_str(),
                    "com.microsoft.vscode" | "code" | "code-oss" | "codium" | "vscodium"
                )
            })
            .find_map(|client| std::fs::read_link(format!("/proc/{}/exe", client.pid)).ok())
            .expect("requires an installed/running VS Code binary");
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("Profile 工具/vscode-user-data");
        let workspace = root.path().join("Multi Codex placement test");
        std::fs::create_dir_all(home.join("User")).unwrap();
        std::fs::create_dir_all(&workspace).unwrap();
        std::fs::write(home.join("User/settings.json"), r#"{"update.mode":"none","telemetry.telemetryLevel":"off","workbench.startupEditor":"none"}"#).unwrap();
        let first_workspace = crate::profiles::create_launch_workspace(
            root.path(),
            home.parent().unwrap(),
            &workspace,
        )
        .unwrap();
        let mut child = Command::new(&executable)
            .arg("--new-window")
            .arg("--disable-extensions")
            .arg("--user-data-dir")
            .arg(&home)
            .arg("--extensions-dir")
            .arg(root.path().join("extensions"))
            .arg(&first_workspace)
            .env("CODEX_HOME", root.path().join("codex-home"))
            .env_remove("ELECTRON_RUN_AS_NODE")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut opened = vec![];
        let result = (|| -> Result<()> {
            let window = find_new_window(&previous, &home)?;
            opened.push(window.clone());
            let destination = (1..=10)
                .rev()
                .find(|id| {
                    before
                        .desktops
                        .iter()
                        .any(|d| d.id == format!("hyprland:{id}") && d.windows.is_empty())
                })
                .ok_or("requires an empty desktop")?;
            move_window(&window, &format!("hyprland:{destination}"))?;
            assert!(hypr_inventory()?
                .desktops
                .iter()
                .any(|d| d.id == format!("hyprland:{destination}")
                    && d.windows.iter().any(|w| w.id == window)));
            // An existing profile process must still produce a distinct, identifiable new window.
            let first_snapshot: HashSet<_> = hypr_inventory()?
                .windows()
                .into_iter()
                .map(|w| w.id)
                .collect();
            let second_workspace = crate::profiles::create_launch_workspace(
                root.path(),
                home.parent().unwrap(),
                &workspace,
            )?;
            assert_ne!(first_workspace, second_workspace);
            Command::new(crate::profiles::resolve_command("code")?)
                .arg("--new-window")
                .arg("--disable-extensions")
                .arg("--user-data-dir")
                .arg(&home)
                .arg("--extensions-dir")
                .arg(root.path().join("extensions"))
                .arg(&second_workspace)
                .env_remove("VSCODE_IPC_HOOK_CLI")
                .env_remove("VSCODE_PID")
                .env_remove("VSCODE_CWD")
                .env_remove("ELECTRON_RUN_AS_NODE")
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|error| error.to_string())?
                .wait()
                .map_err(|error| error.to_string())?;
            let second = find_new_window(&first_snapshot, &home)?;
            opened.push(second.clone());
            assert_ne!(window, second);
            let occupied = before
                .desktops
                .iter()
                .find(|d| !d.windows.is_empty())
                .ok_or("requires an occupied desktop")?;
            move_window(&second, &occupied.id)?;
            println!("Verified disposable VS Code placement on empty desktop {destination} and occupied {}, including a second window of the same profile", occupied.name);
            Ok(())
        })();
        for window in opened {
            let Some(address) = hypr_json::<Vec<HyprClient>>("clients")
                .ok()
                .and_then(|clients| {
                    clients
                        .into_iter()
                        .find(|client| client.window_id() == window)
                })
                .map(|client| client.address)
            else {
                continue;
            };
            assert!(valid_address(&address));
            let _ = crate::process::output(Command::new("hyprctl").args([
                "eval", &format!("local w=hl.get_window('address:{address}'); if w then hl.dispatch(hl.dsp.window.close({{window=w}})) end")
            ]), Duration::from_secs(3));
        }
        // Terminate only the disposable main process we spawned, never a user's VS Code PID.
        let _ = child.kill();
        let _ = child.wait();
        if let Some(current) = before.desktops.iter().find(|d| d.current) {
            let id = current
                .id
                .strip_prefix("hyprland:")
                .unwrap()
                .parse::<i32>()
                .unwrap();
            let _ = crate::process::output(
                Command::new("hyprctl").args([
                    "eval",
                    &format!("hl.dispatch(hl.dsp.focus({{workspace={id}}}))"),
                ]),
                Duration::from_secs(3),
            );
        }
        result.unwrap();
    }

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
        assert_eq!(inventory.desktops.len(), 11);
        let code = inventory
            .desktops
            .iter()
            .find(|d| d.id == "hyprland:2")
            .unwrap();
        let empty = inventory
            .desktops
            .iter()
            .find(|d| d.id == "hyprland:12")
            .unwrap();
        assert_eq!(code.name, "Code");
        assert_eq!(code.windows[0].title, "Project");
        assert!(empty.windows.is_empty());
        assert!(code.current && empty.current);
        assert_eq!(inventory.desktops.iter().filter(|d| d.current).count(), 2);
        assert_eq!(inventory.current_destination(42), Some("hyprland:2"));
        assert_eq!(inventory.current_destination(999), None);
        assert!(inventory.require_destination("hyprland:12").is_ok());
        assert!(inventory.require_destination("hyprland:7").is_ok());
        assert!(inventory.require_destination("hyprland:11").is_err());
    }
    #[test]
    fn unsupported_placement_is_an_error() {
        assert!(unsupported_inventory("macos", "Not validated".into())
            .require_destination("macos:1")
            .is_err());
    }
}
