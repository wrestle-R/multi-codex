//! Read-only discovery of already-running Desktop, CLI and VS Code sessions.
//! No credential reads, extension installation, RPC injection or history scan.
use crate::hud::{Session, Snapshot};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, UpdateKind};

type Cache = Option<(PathBuf, Instant, Vec<Snapshot>)>;
static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();

pub fn publish(root: &Path, engine: &Path, params: &Value) -> Result<Value, String> {
    let pid = params["pid"]
        .as_u64()
        .and_then(|p| u32::try_from(p).ok())
        .ok_or("Invalid Orbit process")?;
    let profile = params["profileId"]
        .as_str()
        .ok_or("Invalid Orbit account")?;
    uuid::Uuid::parse_str(profile).map_err(|_| "Invalid Orbit account")?;
    let profile_home =
        crate::managed_files::checked(root, &root.join("profiles").join(profile), false)?;
    if !profile_home.is_dir() {
        return Err("Orbit account unavailable".into());
    }
    let expected = engine
        .canonicalize()
        .map_err(|_| "Orbit engine unavailable")?;
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always),
    );
    let process = system
        .process(sysinfo::Pid::from_u32(pid))
        .ok_or("Orbit engine has closed")?;
    if process.exe().and_then(|p| p.canonicalize().ok()) != Some(expected) {
        return Err("Orbit engine identity changed".into());
    }
    let threads = params["threads"]
        .as_array()
        .filter(|t| t.len() <= 4096)
        .ok_or("Invalid Orbit sessions")?;
    let mut observer = crate::hud::Observer::new(profile.into(), pid, process.start_time());
    observer.snapshot.client = "vscode".into();
    for thread in threads {
        observer.server(&serde_json::json!({"method":"thread/started","params":{"thread":thread}}));
    }
    crate::hud::atomic_json(
        &root
            .join("hud/instances")
            .join(format!("bridge-{pid}.json")),
        &observer.snapshot,
    )?;
    Ok(serde_json::json!({"published":true}))
}

/// Focus the owning application of a verified live non-Desktop chat. Never
/// launches another client or rewrites authentication to satisfy navigation.
pub fn focus(root: &Path, profile: &str, thread: &str, pid: u32) -> Result<(), String> {
    let snapshot = crate::hud::live_snapshots(root)
        .into_iter()
        .find(|s| {
            s.profile_id == profile
                && s.pid == pid
                && s.client != "desktop"
                && s.sessions.contains_key(thread)
        })
        .ok_or("This chat has closed. Open it again in its original app.")?;
    if crate::hud::process_start(pid) != Some(snapshot.start_time) {
        return Err("This chat has closed.".into());
    }
    #[cfg(unix)]
    if snapshot.client == "vscode" {
        return focus_editor_window(thread);
    }
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    let mut ancestors = Vec::new();
    let mut next = Some(sysinfo::Pid::from_u32(pid));
    for _ in 0..24 {
        let Some(current) = next else { break };
        if ancestors.contains(&current.as_u32()) {
            break;
        }
        ancestors.push(current.as_u32());
        next = system.process(current).and_then(|p| p.parent());
    }
    #[cfg(target_os = "linux")]
    {
        let inventory = crate::desktop_environment::inventory()?;
        let mut windows = Vec::new();
        for desktop in &inventory.desktops {
            for window in &desktop.windows {
                if let Some(distance) = ancestors.iter().position(|p| *p == window.pid) {
                    windows.push((distance, desktop, window));
                }
            }
        }
        windows.sort_by_key(|(distance, _, _)| *distance);
        let Some((distance, desktop, window)) = windows.first() else {
            return Err("The chat is open, but its app window is unavailable. Return to its terminal or VS Code window.".into());
        };
        if windows
            .iter()
            .any(|(d, _, w)| d == distance && w.id != window.id)
        {
            return Err("Several windows share this session's process. Select its terminal or VS Code window directly.".into());
        }
        if crate::desktop_environment::backend() == "hyprland" {
            return crate::desktop_environment::focus_window(&window.id);
        }
        // Existing GNOME/KWin adapters activate a window and its workspace.
        // Passing its current destination preserves its placement.
        return crate::linux_desktops::move_window(
            crate::desktop_environment::backend(),
            &window.id,
            &desktop.id,
        );
    }
    #[cfg(target_os = "macos")]
    {
        return focus_macos(&ancestors);
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    Err("Session focus is unavailable on this platform".into())
}

/// The verified bridge's random session UUID addresses only its owning host.
/// A focus-only socket avoids app-wide activation selecting another Code window.
#[cfg(unix)]
fn focus_editor_window(id: &str) -> Result<(), String> {
    use std::{
        io::Write,
        os::unix::{
            fs::{FileTypeExt, MetadataExt},
            net::UnixStream,
        },
    };
    uuid::Uuid::parse_str(id).map_err(|_| "Invalid Orbit window")?;
    let path = PathBuf::from(format!("/tmp/mcx-orbit-{id}.sock"));
    let meta = fs::symlink_metadata(&path).map_err(|_| "The editor window has closed")?;
    if !meta.file_type().is_socket()
        || meta.uid() != unsafe { libc::geteuid() }
        || meta.mode() & 0o077 != 0
    {
        return Err("The editor window could not be verified".into());
    }
    let mut stream = UnixStream::connect(path).map_err(|_| "The editor window is unavailable")?;
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "Editor focus unavailable")?;
    stream
        .set_write_timeout(Some(Duration::from_secs(3)))
        .map_err(|_| "Editor focus unavailable")?;
    writeln!(stream, "{id}").map_err(|_| "The editor window has closed")?;
    let mut reply = String::new();
    stream
        .take(64)
        .read_to_string(&mut reply)
        .map_err(|_| "The editor did not confirm focus")?;
    if reply == "focused\n" {
        Ok(())
    } else {
        Err("The editor did not confirm focus".into())
    }
}

#[cfg(target_os = "macos")]
fn focus_macos(ancestors: &[u32]) -> Result<(), String> {
    use std::ffi::{c_char, c_void};
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    unsafe {
        let get: unsafe extern "C" fn(*mut c_void, *mut c_void, i32) -> *mut c_void =
            std::mem::transmute(objc_msgSend as *const ());
        let activate: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> bool =
            std::mem::transmute(objc_msgSend as *const ());
        for pid in ancestors {
            let app = get(
                objc_getClass(c"NSRunningApplication".as_ptr()),
                sel_registerName(c"runningApplicationWithProcessIdentifier:".as_ptr()),
                *pid as i32,
            );
            if !app.is_null()
                && activate(app, sel_registerName(c"activateWithOptions:".as_ptr()), 2)
            {
                return Ok(());
            }
        }
    }
    Err("Return to this chat's terminal or VS Code window.".into())
}

pub fn snapshots(root: &Path) -> Vec<Snapshot> {
    let Ok(mut cache) = CACHE.get_or_init(|| Mutex::new(None)).lock() else {
        return vec![];
    };
    if let Some((previous, at, snapshots)) = &*cache {
        if previous == root && at.elapsed() < Duration::from_secs(1) {
            return snapshots.clone();
        }
    }
    let result = discover(root);
    *cache = Some((root.to_owned(), Instant::now(), result.clone()));
    result
}

fn profile_for_home(root: &Path, home: &Path) -> Option<String> {
    let profiles = root.join("profiles").canonicalize().ok()?;
    let actual = home.canonicalize().ok()?;
    let mut parts = actual.strip_prefix(profiles).ok()?.components();
    let id = parts.next()?.as_os_str().to_str()?;
    uuid::Uuid::parse_str(id).ok()?;
    let kind = parts.next()?.as_os_str().to_str()?;
    if kind != "codex-home" && kind != "bridge-runtime" {
        return None;
    }
    Some(id.to_owned())
}

fn discover(root: &Path) -> Vec<Snapshot> {
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::nothing()
            .without_tasks()
            .with_exe(UpdateKind::Always)
            .with_cmd(UpdateKind::Always)
            .with_user(UpdateKind::Always)
            .with_environ(UpdateKind::Always),
    );
    let mut result = vec![];
    for (pid, process) in system.processes() {
        let Some(exe) = process.exe() else { continue };
        if !matches!(
            exe.file_name().and_then(|n| n.to_str()),
            Some("codex" | "codex.exe")
        ) {
            continue;
        }
        #[cfg(unix)]
        if process
            .user_id()
            .is_none_or(|id| **id != unsafe { libc::geteuid() })
        {
            continue;
        }
        let home = process
            .environ()
            .iter()
            .find_map(|s| s.to_str()?.strip_prefix("CODEX_HOME=").map(PathBuf::from));
        let Some(home) = home else { continue };
        let Some(profile_id) = profile_for_home(root, &home) else {
            continue;
        };
        let mut client = "cli";
        if process.cmd().iter().any(|a| a == "app-server") {
            client = "vscode";
            let mut parent = process.parent();
            for _ in 0..12 {
                let Some(p) = parent.and_then(|pid| system.process(pid)) else {
                    break;
                };
                // A Codex tool can itself start another engine. That engine
                // does not become a Desktop client merely through ancestry.
                if p.exe()
                    .and_then(|p| p.file_name())
                    .is_some_and(|p| p == "codex" || p == "code" || p == "Code")
                {
                    break;
                }
                if p.exe()
                    .and_then(|p| p.file_name())
                    .is_some_and(|p| p == "ChatGPT" || p == "Codex")
                {
                    client = "desktop";
                    break;
                }
                parent = p.parent();
            }
        }
        // App-server file descriptors are retained caches, not UI visibility.
        // Desktop has a private window observer; VS Code has the bundled
        // frontend bridge. Only a CLI process supplies native file presence.
        if client != "cli" {
            continue;
        }
        let mut sessions = BTreeMap::new();
        for path in open_rollouts(pid.as_u32()) {
            if let Some(session) = read_session(&path, client) {
                sessions.insert(session.id.clone(), session);
            }
        }
        // Recheck process identity after inspecting its descriptors.
        if !sessions.is_empty()
            && crate::hud::process_start(pid.as_u32()) == Some(process.start_time())
        {
            result.push(Snapshot {
                profile_id,
                pid: pid.as_u32(),
                start_time: process.start_time(),
                sessions,
                limits: None,
                client: client.into(),
            });
        }
    }
    result
}

#[cfg(target_os = "linux")]
fn open_rollouts(pid: u32) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(format!("/proc/{pid}/fd")) else {
        return vec![];
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = fs::read_link(entry.path()).ok()?;
            if !rollout_path(&path) {
                return None;
            }
            // Ignore files briefly read by tools: the engine keeps its active
            // rollout writer open. Never mistake a history read for a live chat.
            let info = fs::read_to_string(format!(
                "/proc/{pid}/fdinfo/{}",
                entry.file_name().to_string_lossy()
            ))
            .ok()?;
            let flags = info
                .lines()
                .find_map(|line| line.strip_prefix("flags:\t"))?;
            if u32::from_str_radix(flags.trim(), 8).ok()? & 3 == 0 {
                return None;
            }
            Some(path)
        })
        .collect()
}

#[cfg(target_os = "macos")]
fn open_rollouts(pid: u32) -> Vec<PathBuf> {
    let Ok(output) = crate::process::output(
        std::process::Command::new("/usr/sbin/lsof").args([
            "-a",
            "-p",
            &pid.to_string(),
            "-Ffan",
            "-n",
            "-P",
        ]),
        Duration::from_secs(2),
    ) else {
        return vec![];
    };
    let mut writable = false;
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            if line.starts_with('f') {
                writable = false;
            }
            if let Some(access) = line.strip_prefix('a') {
                writable = access == "w" || access == "u";
            }
            let path = PathBuf::from(line.strip_prefix('n')?);
            (writable && rollout_path(&path)).then_some(path)
        })
        .collect()
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn open_rollouts(_pid: u32) -> Vec<PathBuf> {
    vec![]
}

fn rollout_path(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("rollout-"))
}

fn read_session(path: &Path, client: &str) -> Option<Session> {
    let file = fs::File::open(path).ok()?;
    let mut reader = BufReader::new(file);
    let mut first = vec![];
    reader
        .by_ref()
        .take(512 * 1024)
        .read_until(b'\n', &mut first)
        .ok()?;
    let meta: Value = serde_json::from_slice(&first).ok()?;
    if meta["type"] != "session_meta" {
        return None;
    }
    let id = meta["payload"]["id"].as_str()?;
    uuid::Uuid::parse_str(id).ok()?;
    if !path
        .file_name()?
        .to_str()?
        .ends_with(&format!("{id}.jsonl"))
    {
        return None;
    }
    if meta["payload"]["source"].is_object() || meta["payload"]["thread_source"].is_object() {
        return None;
    }
    let cwd = meta["payload"]["cwd"].as_str().unwrap_or("").to_owned();
    let end = reader.seek(SeekFrom::End(0)).ok()?;
    let start = end.saturating_sub(256 * 1024);
    reader.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    reader.take(256 * 1024).read_to_end(&mut bytes).ok()?;
    let tail = String::from_utf8_lossy(&bytes);
    let mut status = "unknown";
    for line in tail.lines().skip(usize::from(start > 0)) {
        let Ok(row) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if row["type"] == "event_msg" {
            match row["payload"]["type"].as_str() {
                Some("task_started") => status = "active",
                Some("task_complete" | "turn_aborted") => status = "idle",
                _ => {}
            }
        }
    }
    Some(Session {
        id: id.into(),
        title: match client {
            "cli" => "Codex CLI chat",
            "vscode" => "VS Code chat",
            _ => "Codex Desktop chat",
        }
        .into(),
        cwd,
        status: status.into(),
        attention: None,
        epoch: 0,
        tokens: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn editor_focus_uses_exact_private_window_endpoint() {
        use std::{
            io::Write,
            os::unix::{fs::PermissionsExt, net::UnixListener},
        };
        let id = uuid::Uuid::new_v4().to_string();
        let path = PathBuf::from(format!("/tmp/mcx-orbit-{id}.sock"));
        let listener = UnixListener::bind(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
        assert!(focus_editor_window(&id).is_err());
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        let expected = id.clone();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = String::new();
            BufReader::new(stream.try_clone().unwrap())
                .read_line(&mut request)
                .unwrap();
            assert_eq!(request, format!("{expected}\n"));
            stream.write_all(b"focused\n").unwrap();
        });
        assert!(focus_editor_window(&id).is_ok());
        server.join().unwrap();
        fs::remove_file(path).unwrap();
        assert!(focus_editor_window(&id).is_err());
        assert!(focus_editor_window("../other-window").is_err());
    }
    #[test]
    fn only_exact_managed_homes_and_aliases_match() {
        let root = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let home = root.path().join("profiles").join(&id).join("codex-home");
        fs::create_dir_all(&home).unwrap();
        assert_eq!(profile_for_home(root.path(), &home), Some(id));
        assert_eq!(profile_for_home(root.path(), root.path()), None);
        let wrong = root.path().join("profiles/not-an-account/codex-home");
        fs::create_dir_all(&wrong).unwrap();
        assert_eq!(profile_for_home(root.path(), &wrong), None);
        #[cfg(unix)]
        {
            let alias = root.path().join("alias");
            std::os::unix::fs::symlink(&home, &alias).unwrap();
            assert!(profile_for_home(root.path(), &alias).is_some());
        }
    }
    #[test]
    fn reads_only_allowlisted_metadata_and_terminal_status() {
        let root = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4();
        let path = root.path().join(format!("rollout-fixture-{id}.jsonl"));
        fs::write(&path, format!("{}\n{}\n{}\n", serde_json::json!({"type":"session_meta","payload":{"id":id,"cwd":"/fixture","base_instructions":"PRIVATE_CANARY"}}), serde_json::json!({"type":"event_msg","payload":{"type":"task_started"}}), serde_json::json!({"type":"event_msg","payload":{"type":"task_complete","last_agent_message":"PRIVATE_CANARY"}}))).unwrap();
        let session = read_session(&path, "cli").unwrap();
        assert_eq!(session.status, "idle");
        assert!(!serde_json::to_string(&session)
            .unwrap()
            .contains("PRIVATE_CANARY"));
        let wrong = root.path().join("rollout-wrong.jsonl");
        fs::copy(path, &wrong).unwrap();
        assert!(read_session(&wrong, "cli").is_none());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn saved_history_and_read_only_descriptors_are_not_live() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("rollout-fixture.jsonl");
        fs::write(&path, b"saved history").unwrap();
        let reader = fs::File::open(&path).unwrap();
        assert!(!open_rollouts(std::process::id()).contains(&path));
        let writer = fs::OpenOptions::new().append(true).open(&path).unwrap();
        assert!(open_rollouts(std::process::id()).contains(&path));
        drop(writer);
        assert!(!open_rollouts(std::process::id()).contains(&path));
        drop(reader);
    }
}
