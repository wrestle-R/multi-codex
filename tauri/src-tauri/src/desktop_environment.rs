use serde::Deserialize;
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::profiles::Result;

const WINDOW_WAIT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Deserialize)]
struct HyprClient {
    address: String,
    pid: u32,
}

pub fn is_hyprland() -> bool {
    env::var_os("HYPRLAND_INSTANCE_SIGNATURE").is_some()
}

pub fn configure_main_window() {
    if !is_hyprland() {
        return;
    }
    let pid = std::process::id();
    thread::spawn(move || {
        let deadline = Instant::now() + WINDOW_WAIT;
        while Instant::now() < deadline {
            if let Ok(clients) = read_clients() {
                if let Some(client) = clients.into_iter().find(|client| client.pid == pid) {
                    if valid_address(&client.address) {
                        let _ = run_hyprland_lua(&main_window_lua(&client.address));
                    }
                    return;
                }
            }
            thread::sleep(POLL_INTERVAL);
        }
    });
}

pub fn existing_window_addresses() -> HashSet<String> {
    read_clients()
        .unwrap_or_default()
        .into_iter()
        .map(|client| client.address)
        .collect()
}

pub fn move_new_vscode_window(
    previous: &HashSet<String>,
    vscode_home: &Path,
    workspace: u8,
) -> Result<()> {
    if !(1..=10).contains(&workspace) {
        return Err("Desktop must be between 1 and 10".to_string());
    }
    if !is_hyprland() {
        return Ok(());
    }
    let deadline = Instant::now() + WINDOW_WAIT;
    while Instant::now() < deadline {
        let clients = read_clients()?;
        if let Some(client) = clients.into_iter().find(|client| {
            !previous.contains(&client.address)
                && valid_address(&client.address)
                && process_uses_profile(client.pid, vscode_home)
        }) {
            return run_hyprland_lua(&move_window_lua(&client.address, workspace)).map_err(|_| {
                format!("VS Code opened, but Multi Codex could not move it to desktop {workspace}")
            });
        }
        thread::sleep(POLL_INTERVAL);
    }
    Err(format!(
        "VS Code opened, but its new window could not be found for desktop {workspace}"
    ))
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

fn read_clients() -> Result<Vec<HyprClient>> {
    let output = Command::new("hyprctl")
        .args(["clients", "-j"])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "Hyprland workspace controls are unavailable".to_string())?;
    if !output.status.success() {
        return Err("Hyprland workspace controls are unavailable".to_string());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| "Hyprland returned an invalid window list".to_string())
}

fn process_uses_profile(pid: u32, vscode_home: &Path) -> bool {
    let Ok(command_line) = fs::read(format!("/proc/{pid}/cmdline")) else {
        return false;
    };
    command_line
        .split(|byte| *byte == 0)
        .any(|argument| argument == vscode_home.as_os_str().as_encoded_bytes())
}

fn run_hyprland_lua(lua: &str) -> Result<()> {
    let output = Command::new("hyprctl")
        .args(["eval", lua])
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "Hyprland workspace controls are unavailable".to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err("Hyprland rejected the window operation".to_string())
    }
}

fn valid_address(address: &str) -> bool {
    address.strip_prefix("0x").is_some_and(|digits| {
        !digits.is_empty()
            && digits
                .chars()
                .all(|character| character.is_ascii_hexdigit())
    })
}

fn main_window_lua(address: &str) -> String {
    format!(
        "local w=hl.get_window('address:{address}'); assert(w); \
         hl.dispatch(hl.dsp.window.float({{action='set',window=w}})); \
         hl.dispatch(hl.dsp.window.resize({{x=1180,y=760,'exact',window=w}})); \
         hl.dispatch(hl.dsp.window.center({{window=w}}))"
    )
}

fn move_window_lua(address: &str, workspace: u8) -> String {
    format!(
        "local w=hl.get_window('address:{address}'); assert(w); \
         hl.dispatch(hl.dsp.window.move({{workspace={workspace},follow=true,window=w}}))"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_hyprland_addresses_before_embedding_them() {
        assert!(valid_address("0x12aB90"));
        assert!(!valid_address("0x12'; os.execute('bad')"));
        assert!(!valid_address("12ab"));
    }

    #[test]
    fn main_window_command_targets_the_app_and_sets_popup_geometry() {
        let lua = main_window_lua("0xabc123");
        assert!(lua.contains("address:0xabc123"));
        assert!(lua.contains("action='set'"));
        assert!(lua.contains("x=1180,y=760"));
        assert!(lua.contains("center"));
    }

    #[test]
    fn workspace_command_targets_only_the_new_window() {
        let lua = move_window_lua("0xabc123", 7);
        assert!(lua.contains("address:0xabc123"));
        assert!(lua.contains("workspace=7"));
        assert!(lua.contains("follow=true"));
    }
}
