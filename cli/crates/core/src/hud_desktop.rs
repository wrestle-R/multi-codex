//! Read-only Desktop window visibility. A loaded engine thread is not an open
//! UI chat. Observe only a newly launched managed browser on random loopback.
//! The observer owns neither the browser lifetime nor any authentication.
use crate::{hud, profiles::Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

// Only public app-action attributes, never transcript text, tokens or React
// internals. Unknown UI versions fail closed instead of counting cached chats.
pub const SELECTED_THREADS: &str = r#"(()=>{
 const composer = document.querySelector('[data-above-composer-conversation-id]');
 const current = composer?.getAttribute('data-above-composer-conversation-id');
 if (current) return [current];
 return Array.from(document.querySelectorAll('[data-app-action-sidebar-thread-selected="true"][data-app-action-sidebar-thread-kind="local"][data-app-action-sidebar-thread-host-id="local"]'),e=>e.getAttribute('data-app-action-sidebar-thread-id')).filter(v=>typeof v==='string'&&v.startsWith('local:')).map(v=>v.slice(6));
})()"#;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Visibility {
    pub profile_id: String,
    pub pid: u32,
    pub start_time: u64,
    pub observed_at: u64,
    pub threads: BTreeSet<String>,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
pub(crate) fn states(root: &Path) -> Vec<Visibility> {
    let directory = root.join("hud/desktop");
    if crate::managed_files::checked(root, &directory, false).is_err() {
        return vec![];
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return vec![];
    };
    let mut result = Vec::new();
    for entry in entries.flatten().take(128) {
        let path = entry.path();
        if path.extension().is_none_or(|e| e != "json") {
            continue;
        }
        let bytes = hud::read_private(&path).ok();
        let Some(mut state) = bytes.and_then(|b| serde_json::from_slice::<Visibility>(&b).ok())
        else {
            continue;
        };
        if uuid::Uuid::parse_str(&state.profile_id).is_err()
            || now().saturating_sub(state.observed_at) > 5
            || state.observed_at > now().saturating_add(1)
            || hud::process_start(state.pid) != Some(state.start_time)
        {
            continue;
        }
        state.threads.retain(|id| uuid::Uuid::parse_str(id).is_ok());
        result.push(state);
    }
    result
}
pub fn visible(root: &Path, profile: &str) -> Option<BTreeSet<String>> {
    let mut found = false;
    let mut threads = BTreeSet::new();
    for state in states(root).into_iter().filter(|s| s.profile_id == profile) {
        found = true;
        threads.extend(state.threads);
    }
    found.then_some(threads)
}

// DevToolsActivePort belongs to the just-launched private user-data directory.
// Never accept a hostname, another browser, or an arbitrary inspector URL.
fn browser_endpoint(bytes: &[u8]) -> Result<(std::net::SocketAddr, String)> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Invalid Desktop observer endpoint")?;
    let mut lines = text.lines();
    let port = lines
        .next()
        .and_then(|p| p.parse::<u16>().ok())
        .filter(|p| *p != 0)
        .ok_or("Invalid Desktop observer port")?;
    let route = lines.next().ok_or("Invalid Desktop observer endpoint")?;
    let token = route
        .strip_prefix("/devtools/browser/")
        .ok_or("Invalid Desktop observer endpoint")?;
    if uuid::Uuid::parse_str(token).is_err() || lines.next().is_some() {
        return Err("Invalid Desktop observer endpoint".into());
    }
    Ok((
        std::net::SocketAddr::from(([127, 0, 0, 1], port)),
        format!("ws://127.0.0.1:{port}{route}"),
    ))
}

#[cfg(unix)]
pub fn launch(args: &[std::ffi::OsString]) -> Result<()> {
    use std::{
        fs,
        net::TcpStream,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };
    use tungstenite::{protocol::WebSocketConfig, Message, WebSocket};
    if args.len() < 3 {
        return Err("Invalid Desktop visibility invocation".into());
    }
    let binary = Path::new(&args[0]);
    let root = Path::new(&args[1]);
    let profile = args[2].to_str().ok_or("Invalid Orbit account")?;
    if !binary.is_absolute()
        || !crate::profiles::is_executable_file(binary)
        || !root.is_absolute()
        || uuid::Uuid::parse_str(profile).is_err()
    {
        return Err("Invalid Desktop visibility paths".into());
    }
    let desktop = crate::managed_files::checked(
        root,
        &root.join("profiles").join(profile).join("desktop-data"),
        false,
    )?;
    let configured =
        std::env::var_os("CODEX_ELECTRON_USER_DATA_PATH").ok_or("Desktop profile unavailable")?;
    if fs::canonicalize(configured).ok() != fs::canonicalize(&desktop).ok() {
        return Err("Desktop visibility profile does not match".into());
    }
    let endpoint_file =
        crate::managed_files::checked(root, &desktop.join("DevToolsActivePort"), true)?;
    match fs::remove_file(&endpoint_file) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(_) => return Err("Could not prepare Desktop observer endpoint".into()),
    }
    let mut command = Command::new(binary);
    // Pipe mode closes this runtime's browser on observer EOF. A loopback
    // observer can disconnect without closing any chats.
    command
        .arg("--remote-debugging-address=127.0.0.1")
        .arg("--remote-debugging-port=0")
        .args(
            args[3..]
                .iter()
                .filter(|arg| !arg.to_string_lossy().starts_with("--remote-debugging-")),
        )
        .stdin(Stdio::null());
    let mut child = command
        .spawn()
        .map_err(|_| "Could not start Codex Desktop")?;
    let pid = child.id();
    let start = hud::process_start(pid).unwrap_or(0);
    let path = root.join("hud/desktop").join(format!("{pid}.json"));
    let mut socket: Option<WebSocket<TcpStream>> = None;
    let mut sequence = 0u64;
    let mut sessions = std::collections::BTreeMap::<String, String>::new();
    while child
        .try_wait()
        .map_err(|_| "Desktop process unavailable")?
        .is_none()
    {
        if socket.is_none() {
            let connected = (|| -> Result<WebSocket<TcpStream>> {
                let bytes = hud::read_private(&endpoint_file)?;
                if bytes.len() > 1024 {
                    return Err("Invalid Desktop observer endpoint".into());
                }
                let (address, url) = browser_endpoint(&bytes)?;
                let stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
                    .map_err(|_| "Desktop observer unavailable")?;
                stream
                    .set_read_timeout(Some(Duration::from_millis(500)))
                    .map_err(|_| "Desktop observer unavailable")?;
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .map_err(|_| "Desktop observer unavailable")?;
                let config = WebSocketConfig::default()
                    .max_message_size(Some(1024 * 1024))
                    .max_frame_size(Some(1024 * 1024));
                let (socket, _) =
                    tungstenite::client::client_with_config(url.as_str(), stream, Some(config))
                        .map_err(|_| "Desktop observer unavailable")?;
                Ok(socket)
            })();
            if let Ok(connected) = connected {
                socket = Some(connected);
                sessions.clear();
            }
        }
        let observed = (|| -> Result<BTreeSet<String>> {
            let socket = socket.as_mut().ok_or("Desktop visibility unavailable")?;
            let mut request = |method: &str,
                               params: serde_json::Value,
                               session: Option<&str>|
             -> Result<serde_json::Value> {
                sequence += 1;
                let mut message =
                    serde_json::json!({"id":sequence,"method":method,"params":params});
                if let Some(id) = session {
                    message["sessionId"] = id.into();
                }
                socket
                    .send(Message::Text(message.to_string().into()))
                    .map_err(|_| "Desktop visibility disconnected")?;
                let deadline = Instant::now() + Duration::from_secs(4);
                while Instant::now() < deadline {
                    match socket.read() {
                        Ok(Message::Text(bytes)) => {
                            let response: serde_json::Value = serde_json::from_str(&bytes)
                                .map_err(|_| "Invalid Desktop visibility message")?;
                            if response["id"].as_u64() == Some(sequence) {
                                if response.get("error").is_some() {
                                    return Err("Desktop visibility unavailable".into());
                                }
                                return Ok(response["result"].clone());
                            }
                        }
                        Ok(Message::Close(_)) => {
                            return Err("Desktop visibility disconnected".into())
                        }
                        Ok(_) => {}
                        Err(tungstenite::Error::Io(e))
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::TimedOut
                                    | std::io::ErrorKind::WouldBlock
                                    | std::io::ErrorKind::Interrupted
                            ) => {}
                        Err(_) => return Err("Desktop visibility disconnected".into()),
                    }
                }
                Err("Desktop visibility timed out".into())
            };
            let targets = request("Target.getTargets", serde_json::json!({}), None)?;
            let targets = targets["targetInfos"]
                .as_array()
                .ok_or("Invalid Desktop windows")?;
            let mut threads = BTreeSet::new();
            for target in targets
                .iter()
                .filter(|t| {
                    t["type"] == "page"
                        && t["url"]
                            .as_str()
                            .is_some_and(|url| url.starts_with("app://"))
                })
                .take(16)
            {
                let target_id = target["targetId"]
                    .as_str()
                    .ok_or("Desktop window unavailable")?;
                if !sessions.contains_key(target_id) {
                    let attached = request(
                        "Target.attachToTarget",
                        serde_json::json!({"targetId":target_id,"flatten":true}),
                        None,
                    )?;
                    let session = attached["sessionId"]
                        .as_str()
                        .ok_or("Desktop window unavailable")?;
                    sessions.insert(target_id.into(), session.into());
                }
                let reply = request(
                    "Runtime.evaluate",
                    serde_json::json!({"expression":SELECTED_THREADS,"returnByValue":true}),
                    Some(&sessions[target_id]),
                )?;
                if reply.get("exceptionDetails").is_some() {
                    return Err("Desktop visibility unavailable".into());
                }
                let values = reply["result"]["value"]
                    .as_array()
                    .ok_or("Desktop visibility unavailable")?;
                threads.extend(
                    values
                        .iter()
                        .filter_map(|v| v.as_str())
                        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                        .take(16)
                        .map(str::to_owned),
                );
            }
            sessions.retain(|target, _| targets.iter().any(|t| t["targetId"] == target.as_str()));
            Ok(threads)
        })();
        if let Ok(threads) = observed {
            let state = Visibility {
                profile_id: profile.into(),
                pid,
                start_time: start,
                observed_at: now(),
                threads,
            };
            let _ = hud::atomic_json(&path, &state);
        } else {
            socket = None;
        }
        std::thread::sleep(Duration::from_millis(750));
    }
    let _ = fs::remove_file(path);
    Ok(())
}
#[cfg(not(unix))]
pub fn launch(_: &[std::ffi::OsString]) -> Result<()> {
    Err("Desktop visibility is unavailable on this platform".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_idle_chat_survives_backend_eviction_without_inventing_usage() {
        let root = tempfile::tempdir().unwrap();
        let profile = uuid::Uuid::new_v4().to_string();
        let thread = uuid::Uuid::new_v4().to_string();
        let pid = std::process::id();
        let path = root.path().join("hud/desktop/visible.json");
        let mut state = Visibility {
            profile_id: profile,
            pid,
            start_time: hud::process_start(pid).unwrap(),
            observed_at: now(),
            threads: BTreeSet::from([thread.clone()]),
        };
        hud::atomic_json(&path, &state).unwrap();
        let snapshots = hud::live_snapshots(root.path());
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].sessions[&thread].status, "unknown");
        assert!(snapshots[0].sessions[&thread].tokens.is_none());
        state.threads.clear();
        hud::atomic_json(&path, &state).unwrap();
        assert!(hud::live_snapshots(root.path()).is_empty());
    }
    #[test]
    fn inspector_discovery_cannot_redirect_outside_loopback_or_to_another_target() {
        let token = uuid::Uuid::new_v4();
        let valid = format!("34567\n/devtools/browser/{token}");
        let (address, url) = browser_endpoint(valid.as_bytes()).unwrap();
        assert_eq!(address.to_string(), "127.0.0.1:34567");
        assert_eq!(
            url,
            format!("ws://127.0.0.1:34567/devtools/browser/{token}")
        );
        for invalid in [
            "0\n/devtools/browser/id",
            "70000\n/devtools/browser/id",
            "34567\nws://other/browser/id",
            "34567\n/devtools/page/id",
            "34567\n/devtools/browser/../escape",
            "34567\n/devtools/browser/id\nextra",
        ] {
            assert!(browser_endpoint(invalid.as_bytes()).is_err());
        }
    }
    #[test]
    fn empty_visibility_is_authoritative_but_stale_or_recycled_process_is_not() {
        let root = tempfile::tempdir().unwrap();
        let profile = uuid::Uuid::new_v4().to_string();
        let pid = std::process::id();
        let path = root.path().join("hud/desktop/test.json");
        let mut state = Visibility {
            profile_id: profile.clone(),
            pid,
            start_time: hud::process_start(pid).unwrap(),
            observed_at: now(),
            threads: BTreeSet::new(),
        };
        hud::atomic_json(&path, &state).unwrap();
        assert_eq!(visible(root.path(), &profile), Some(BTreeSet::new()));
        state.observed_at = now() - 10;
        hud::atomic_json(&path, &state).unwrap();
        assert!(visible(root.path(), &profile).is_none());
        state.observed_at = now();
        state.start_time += 1;
        hud::atomic_json(&path, &state).unwrap();
        assert!(visible(root.path(), &profile).is_none());
    }
}
