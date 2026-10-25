//! Passive Desktop telemetry. Never reads credentials or changes an RPC payload.
use crate::profiles::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HudSettings {
    pub enabled: bool,
    pub size: u32,
    pub opacity: f64,
    pub monitor: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub only_open: bool,
    pub animation: bool,
    pub reduced_motion: bool,
    pub sound: bool,
}
impl Default for HudSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            size: 56,
            opacity: 0.95,
            monitor: None,
            x: None,
            y: None,
            only_open: false,
            animation: true,
            reduced_motion: false,
            sound: false,
        }
    }
}
impl HudSettings {
    pub fn validate(&self) -> Result<()> {
        if !(32..=100).contains(&self.size)
            || !self.opacity.is_finite()
            || !(0.35..=1.0).contains(&self.opacity)
            || self.x.is_some_and(|v| !v.is_finite())
            || self.y.is_some_and(|v| !v.is_finite())
            || self.monitor.as_ref().is_some_and(|v| v.len() > 256)
        {
            return Err("Invalid HUD settings".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Avatar {
    pub kind: String,
    pub color: String,
    pub seed: u32,
}
pub fn avatar(id: &str) -> Avatar {
    // Identity derives from the immutable public account ID, not its name,
    // position, theme or credentials. Existing accounts get the same identity.
    let hash = Sha256::digest(id.as_bytes());
    let kinds = [
        "clover", "flower", "ghost", "square", "cloud", "cat", "pebble", "blob", "star", "drop",
        "circle", "pill", "alien", "droid", "mech", "hexagon", "triangle", "puddle",
    ];
    let colors = [
        "#60c8df", "#73d8a6", "#c6b9e8", "#de8abc", "#e2b769", "#90acf1",
    ];
    Avatar {
        kind: kinds[hash[0] as usize % kinds.len()].into(),
        color: colors[hash[1] as usize % colors.len()].into(),
        seed: u32::from_le_bytes(hash[2..6].try_into().unwrap()),
    }
}
pub fn load_settings(root: &Path) -> Result<HudSettings> {
    let path = root.join("hud/settings.json");
    match read_private(&path) {
        Ok(bytes) => {
            let settings: HudSettings =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid saved HUD settings")?;
            settings.validate()?;
            Ok(settings)
        }
        Err(_) if !path.exists() => Ok(HudSettings::default()),
        Err(error) => Err(error),
    }
}
pub fn save_settings(root: &Path, settings: &HudSettings) -> Result<()> {
    settings.validate()?;
    atomic_json(&root.join("hud/settings.json"), settings)
}
pub(crate) fn read_private(path: &Path) -> Result<Vec<u8>> {
    let info = fs::symlink_metadata(path).map_err(|_| "HUD state is unavailable")?;
    if !info.is_file() || info.file_type().is_symlink() || info.len() > 2 * 1024 * 1024 {
        return Err("Invalid HUD state file".into());
    }
    fs::read(path).map_err(|_| "Could not read HUD state".into())
}
pub fn atomic_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().ok_or("Missing HUD directory")?;
    let root = path
        .ancestors()
        .find(|ancestor| ancestor.file_name().is_some_and(|name| name == "hud"))
        .and_then(Path::parent)
        .ok_or("HUD state must remain in its private directory")?;
    crate::managed_files::directory(root, parent)?;
    crate::managed_files::checked(root, path, true)?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|_| "Could not write HUD state")?;
        crate::file_security::protect(&temporary, false)?;
        serde_json::to_writer(&mut file, value).map_err(|_| "Could not encode HUD state")?;
        file.sync_all().map_err(|_| "Could not flush HUD state")?;
        fs::rename(&temporary, path).map_err(|_| "Could not commit HUD state")?;
        #[cfg(unix)]
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|_| "Could not flush HUD directory")?;
        Ok(())
    })();
    let _ = fs::remove_file(temporary);
    result
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub cwd: String,
    pub status: String,
    pub attention: Option<String>,
    pub epoch: u64,
    pub tokens: Option<Value>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub profile_id: String,
    pub pid: u32,
    pub start_time: u64,
    pub sessions: BTreeMap<String, Session>,
    pub limits: Option<Value>,
    #[serde(default = "desktop_client")]
    pub client: String,
}
fn desktop_client() -> String {
    "desktop".into()
}
#[derive(Default)]
pub struct Observer {
    pub snapshot: Snapshot,
    requests: BTreeMap<String, (String, Option<String>)>,
    approvals: BTreeMap<String, String>,
    last_reply: BTreeMap<String, String>,
}
fn key(value: &Value) -> String {
    value.to_string()
}
fn identity(value: &Value) -> Option<String> {
    value
        .as_str()
        .filter(|v| uuid::Uuid::parse_str(v).is_ok())
        .map(str::to_owned)
}
pub(crate) fn title(value: &Value) -> String {
    value
        .as_str()
        .unwrap_or("Codex chat")
        .split_whitespace()
        .map(|word| {
            if word.starts_with("sk-") || word.starts_with("sk_") || word.len() > 100 {
                "[redacted]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(120)
        .collect()
}
fn clean_limits(value: &Value) -> Value {
    let window = |v: &Value| json!({"usedPercent":v["usedPercent"].as_f64(),"windowDurationMins":v["windowDurationMins"].as_u64(),"resetsAt":v["resetsAt"].as_u64()});
    json!({"primary":window(&value["primary"]),"secondary":window(&value["secondary"]),"planType":value["planType"].as_str()})
}
impl Observer {
    pub fn new(profile: String, pid: u32, start_time: u64) -> Self {
        Self {
            snapshot: Snapshot {
                profile_id: profile,
                pid,
                start_time,
                client: desktop_client(),
                ..Default::default()
            },
            ..Default::default()
        }
    }
    fn loaded(&mut self, thread: &Value) {
        let Some(id) = identity(&thread["id"]) else {
            return;
        };
        let entry = self.snapshot.sessions.entry(id.clone()).or_insert(Session {
            id,
            title: "Codex chat".into(),
            cwd: String::new(),
            status: "idle".into(),
            attention: None,
            epoch: 0,
            tokens: None,
        });
        // Preview is the first user message, not an allowlisted chat name.
        // Keep it out of HUD files even for resumed or forked unnamed chats.
        entry.title = title(&thread["name"]);
        entry.cwd = thread["cwd"]
            .as_str()
            .unwrap_or("")
            .chars()
            .take(2048)
            .collect();
        if let Some(status) = thread["status"]["type"].as_str() {
            entry.status = status.into();
        }
    }
    fn alert(&mut self, id: &str, reason: &str) {
        if let Some(session) = self.snapshot.sessions.get_mut(id) {
            session.attention = Some(reason.into());
            session.epoch += 1;
        }
    }
    pub fn client(&mut self, message: &Value) {
        if let (Some(method), Some(id)) = (message["method"].as_str(), message.get("id")) {
            if self.requests.len() > 2048 {
                self.requests.clear();
            }
            self.requests.insert(
                key(id),
                (method.into(), identity(&message["params"]["threadId"])),
            );
        } else if let Some(id) = message.get("id") {
            if let Some(thread) = self.approvals.remove(&key(id)) {
                if !self.approvals.values().any(|v| v == &thread) {
                    if let Some(session) = self.snapshot.sessions.get_mut(&thread) {
                        session.attention = None;
                    }
                }
            }
        }
    }
    pub fn server(&mut self, message: &Value) {
        if message.get("method").is_none() {
            if let Some((method, thread)) = message
                .get("id")
                .and_then(|id| self.requests.remove(&key(id)))
            {
                if message.get("error").is_some() {
                    return;
                }
                match method.as_str() {
                    "thread/start" | "thread/resume" | "thread/fork" => {
                        self.loaded(&message["result"]["thread"])
                    }
                    "thread/unsubscribe" | "thread/delete" => {
                        if let Some(id) = thread {
                            self.snapshot.sessions.remove(&id);
                        }
                    }
                    "account/rateLimits/read" => {
                        self.snapshot.limits = Some(clean_limits(&message["result"]["rateLimits"]))
                    }
                    _ => {}
                }
            }
            return;
        }
        let method = message["method"].as_str().unwrap_or("");
        let params = &message["params"];
        if method == "thread/closed" || method == "thread/deleted" {
            if let Some(id) = identity(&params["threadId"]) {
                self.snapshot.sessions.remove(&id);
                self.last_reply.remove(&id);
                self.approvals.retain(|_, thread| thread != &id);
            }
            return;
        }
        if method == "thread/started" {
            self.loaded(&params["thread"]);
            return;
        }
        if method == "account/rateLimits/updated" {
            self.snapshot.limits = Some(clean_limits(&params["rateLimits"]));
            return;
        }
        if method == "serverRequest/resolved" {
            if let Some(thread) = self.approvals.remove(&key(&params["requestId"])) {
                if !self.approvals.values().any(|v| v == &thread) {
                    if let Some(session) = self.snapshot.sessions.get_mut(&thread) {
                        session.attention = None;
                    }
                }
            }
            return;
        }
        let Some(id) = identity(&params["threadId"]).or_else(|| identity(&params["thread_id"]))
        else {
            return;
        };
        if message.get("id").is_some()
            && (method.contains("requestApproval")
                || method.contains("requestUserInput")
                || method.contains("elicitation"))
        {
            self.approvals.insert(key(&message["id"]), id.clone());
            self.alert(
                &id,
                if method.contains("requestUserInput") {
                    "question"
                } else {
                    "approval"
                },
            );
            return;
        }
        match method {
            "turn/started" => {
                self.last_reply.remove(&id);
                if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                    session.status = "active".into();
                    session.attention = None;
                }
            }
            "thread/status/changed" => {
                if params["status"]["type"] == "notLoaded" {
                    self.snapshot.sessions.remove(&id);
                    return;
                }
                if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                    session.status = params["status"]["type"]
                        .as_str()
                        .unwrap_or("unknown")
                        .into();
                }
            }
            "thread/name/updated" => {
                if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                    session.title = title(&params["threadName"]);
                }
            }
            "item/completed" => {
                if params["item"]["type"] == "agentMessage" {
                    self.last_reply.insert(
                        id,
                        params["item"]["text"]
                            .as_str()
                            .unwrap_or("")
                            .chars()
                            .take(16384)
                            .collect(),
                    );
                }
            }
            "turn/completed" => {
                if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                    session.status = "idle".into();
                }
                let text = self.last_reply.remove(&id).unwrap_or_default();
                if needs_user(&text) {
                    self.alert(&id, "mayNeedYou");
                } else if params["turn"]["status"] == "failed" {
                    self.alert(&id, "error");
                }
            }
            "thread/tokenUsage/updated" => {
                if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                    let usage = &params["tokenUsage"];
                    let total = &usage["total"];
                    session.tokens = Some(
                        json!({"total":total["totalTokens"].as_u64(),"input":total["inputTokens"].as_u64(),"output":total["outputTokens"].as_u64(),"contextWindow":usage["modelContextWindow"].as_u64()}),
                    );
                }
            }
            "serverRequest/resolved" => {
                self.approvals.remove(&key(&params["requestId"]));
                if !self.approvals.values().any(|v| v == &id) {
                    if let Some(session) = self.snapshot.sessions.get_mut(&id) {
                        session.attention = None;
                    }
                }
            }
            "thread/closed" | "thread/deleted" => {
                self.snapshot.sessions.remove(&id);
            }
            _ => {}
        }
    }
}
pub fn needs_user(text: &str) -> bool {
    let text = text.to_lowercase();
    text.contains('?')
        || [
            "please ",
            "you need to",
            "you'll need",
            "your turn",
            "let me know",
            "log in",
            "login",
            "sign in",
            "api key",
            "update the token",
            "paste ",
            "report back",
            "confirm ",
            "waiting for you",
            "can you",
            "could you",
            "run this",
            "run the following",
        ]
        .iter()
        .any(|phrase| text.contains(phrase))
}
pub fn process_start(pid: u32) -> Option<u64> {
    let mut system = sysinfo::System::new();
    system.refresh_processes_specifics(
        sysinfo::ProcessesToUpdate::Some(&[sysinfo::Pid::from_u32(pid)]),
        true,
        sysinfo::ProcessRefreshKind::nothing().without_tasks(),
    );
    system
        .process(sysinfo::Pid::from_u32(pid))
        .map(|p| p.start_time())
}
pub fn snapshots(root: &Path) -> Vec<Snapshot> {
    let mut result = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("hud/instances")) else {
        return result;
    };
    for entry in entries.flatten() {
        if entry
            .path()
            .extension()
            .is_none_or(|extension| extension != "json")
        {
            continue;
        }
        if let Ok(bytes) = read_private(&entry.path()) {
            if let Ok(snapshot) = serde_json::from_slice::<Snapshot>(&bytes) {
                if process_start(snapshot.pid) == Some(snapshot.start_time) {
                    let mut snapshot = snapshot;
                    snapshot.sessions.retain(|_, s| s.status != "notLoaded");
                    result.push(snapshot);
                }
            }
        }
    }
    result
}

/// Existing sessions are discovered without restarting or altering a client.
/// Saved history is never scanned: only rollouts held open by a live Codex
/// engine contribute. Rich proxy events take precedence for the same process.
pub fn live_snapshots(root: &Path) -> Vec<Snapshot> {
    let mut result = snapshots(root);
    for native in crate::hud_sessions::snapshots(root) {
        if result
            .iter_mut()
            .any(|s| s.pid == native.pid && s.start_time == native.start_time)
        {
            // Native engines can retain rollout writer descriptors after an
            // unsubscribe. An authoritative empty proxy/bridge snapshot wins.
            continue;
        } else {
            result.push(native);
        }
    }
    result.sort_by_key(|s| std::cmp::Reverse((s.start_time, s.pid)));
    // UI selection owns presence; backend residency only enriches it. An
    // idle visible chat remains open when the engine evicts its cached state.
    let states = crate::hud_desktop::states(root);
    let mut visible = BTreeMap::<String, std::collections::BTreeSet<String>>::new();
    for state in &states {
        visible
            .entry(state.profile_id.clone())
            .or_default()
            .extend(state.threads.iter().cloned());
    }
    let mut matched = std::collections::BTreeSet::new();
    for snapshot in result.iter_mut().filter(|s| s.client == "desktop") {
        snapshot.sessions.retain(|id, _| {
            visible
                .get(&snapshot.profile_id)
                .is_some_and(|ids| ids.contains(id))
                && matched.insert((snapshot.profile_id.clone(), id.clone()))
        });
    }
    for state in states {
        let sessions = state
            .threads
            .into_iter()
            .filter(|id| matched.insert((state.profile_id.clone(), id.clone())))
            .map(|id| {
                (
                    id.clone(),
                    Session {
                        id,
                        title: "Codex Desktop chat".into(),
                        cwd: String::new(),
                        status: "unknown".into(),
                        attention: None,
                        epoch: 0,
                        tokens: None,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();
        if !sessions.is_empty() {
            result.push(Snapshot {
                profile_id: state.profile_id,
                pid: state.pid,
                start_time: state.start_time,
                client: "desktop".into(),
                sessions,
                limits: None,
            });
        }
    }
    result
}
/// Entrypoint for CODEX_CLI_PATH. RPC bytes are forwarded verbatim in both
/// directions; only allowlisted telemetry fields are written to private disk.
pub fn proxy(args: &[std::ffi::OsString]) -> Result<()> {
    if args.len() < 3 {
        return Err("Invalid Desktop observer invocation".into());
    }
    let engine = PathBuf::from(&args[0]);
    let root = PathBuf::from(&args[1]);
    let profile = args[2].to_str().ok_or("Invalid observer account")?;
    if !engine.is_absolute()
        || !crate::profiles::is_executable_file(&engine)
        || !root.is_absolute()
        || uuid::Uuid::parse_str(profile).is_err()
    {
        return Err("Invalid Desktop observer paths".into());
    }
    if !args[3..].iter().any(|arg| arg == "app-server") {
        // Desktop also asks this executable for its version/capabilities. Fast
        // commands can exit before a process-start timestamp is observable.
        let status = std::process::Command::new(engine)
            .args(&args[3..])
            .status()
            .map_err(|_| "Could not run Desktop engine")?;
        std::process::exit(status.code().unwrap_or(1));
    }
    let mut child = std::process::Command::new(engine)
        .args(&args[3..])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|_| "Could not start Desktop engine")?;
    let observer = Arc::new(Mutex::new(Observer::new(
        profile.into(),
        child.id(),
        process_start(child.id()).ok_or("Desktop engine identity unavailable")?,
    )));
    let path = root
        .join("hud/instances")
        .join(format!("{}.json", uuid::Uuid::new_v4()));
    let mut input = child.stdin.take().unwrap();
    let output = child.stdout.take().unwrap();
    let write_state = observer.clone();
    let state_path = path.clone();
    let output_thread = std::thread::spawn(move || -> std::io::Result<()> {
        let mut reader = BufReader::new(output);
        let mut stdout = std::io::stdout().lock();
        let mut line = Vec::new();
        let mut previous = String::new();
        while reader.read_until(b'\n', &mut line)? != 0 {
            stdout.write_all(&line)?;
            stdout.flush()?;
            if line.len() <= 2 * 1024 * 1024 {
                if let Ok(message) = serde_json::from_slice::<Value>(&line) {
                    if let Ok(mut state) = write_state.lock() {
                        state.server(&message);
                        if let Ok(encoded) = serde_json::to_string(&state.snapshot) {
                            if encoded != previous {
                                let _ = atomic_json(&state_path, &state.snapshot);
                                previous = encoded;
                            }
                        }
                    }
                }
            }
            line.clear();
        }
        Ok(())
    });
    std::thread::spawn(move || {
        let mut reader = BufReader::new(std::io::stdin());
        let mut line = Vec::new();
        while reader.read_until(b'\n', &mut line).unwrap_or(0) != 0 {
            if line.len() <= 2 * 1024 * 1024 {
                if let Ok(message) = serde_json::from_slice::<Value>(&line) {
                    if let Ok(mut state) = observer.lock() {
                        state.client(&message);
                    }
                }
            }
            if input.write_all(&line).and_then(|_| input.flush()).is_err() {
                break;
            }
            line.clear();
        }
    });
    let status = child
        .wait()
        .map_err(|_| "Desktop engine exited unexpectedly")?;
    let _ = output_thread.join();
    let _ = fs::remove_file(path);
    std::process::exit(status.code().unwrap_or(1));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_and_settings_are_stable_and_private() {
        assert_eq!(
            serde_json::to_value(avatar("a")).unwrap(),
            serde_json::to_value(avatar("a")).unwrap()
        );
        assert_ne!(avatar("a").seed, avatar("b").seed);
        let dir = tempfile::tempdir().unwrap();
        let mut settings = HudSettings::default();
        settings.size = 72;
        save_settings(dir.path(), &settings).unwrap();
        assert_eq!(load_settings(dir.path()).unwrap().size, 72);
        settings.opacity = f64::NAN;
        assert!(save_settings(dir.path(), &settings).is_err());
    }
    #[test]
    fn live_sessions_and_attention_are_separate_from_history_and_credentials() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = Observer::default();
        state.client(&json!({"id":1,"method":"thread/list","params":{}}));
        state.server(&json!({"id":1,"result":{"data":[{"id":id}]}}));
        assert!(state.snapshot.sessions.is_empty());
        state.server(&json!({"method":"thread/started","params":{"thread":{"id":id,"name":"Fixture","cwd":"/tmp"}}}));
        state.server(&json!({"id":2,"method":"item/tool/requestUserInput","params":{"threadId":id,"questions":[{"secret":"NEVER_PERSIST"}]}}));
        assert_eq!(
            state.snapshot.sessions[&id].attention.as_deref(),
            Some("question")
        );
        state.client(&json!({"id":2,"result":{"answer":"NEVER_PERSIST"}}));
        assert!(state.snapshot.sessions[&id].attention.is_none());
        state.server(&json!({"method":"item/completed","params":{"threadId":id,"item":{"type":"agentMessage","text":"Please log in with NEVER_PERSIST and report back"}}}));
        state.server(&json!({"method":"turn/completed","params":{"threadId":id,"turn":{"status":"completed"}}}));
        assert_eq!(
            state.snapshot.sessions[&id].attention.as_deref(),
            Some("mayNeedYou")
        );
        assert_eq!(state.snapshot.sessions[&id].epoch, 2);
        assert!(!serde_json::to_string(&state.snapshot)
            .unwrap()
            .contains("NEVER_PERSIST"));
        state.client(&json!({"id":3,"method":"thread/unsubscribe","params":{"threadId":id}}));
        state.server(&json!({"id":3,"result":{}}));
        assert!(state.snapshot.sessions.is_empty());
    }
    #[test]
    fn unnamed_resumed_chats_never_persist_message_previews() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = Observer::default();
        state.client(&json!({"id":1,"method":"thread/resume","params":{"threadId":id}}));
        state.server(&json!({"id":1,"result":{"thread":{"id":id,"name":null,"preview":"NEVER_PERSIST_THIS_MESSAGE","cwd":"/tmp"}}}));
        assert_eq!(state.snapshot.sessions[&id].title, "Codex chat");
        assert!(!serde_json::to_string(&state.snapshot)
            .unwrap()
            .contains("NEVER_PERSIST"));
        state.server(&json!({"method":"thread/name/updated","params":{"threadId":id,"threadName":"Named fixture"}}));
        assert_eq!(state.snapshot.sessions[&id].title, "Named fixture");
    }
    #[test]
    fn closed_and_unloaded_chats_do_not_remain_in_only_open_accounts() {
        let id = uuid::Uuid::new_v4().to_string();
        let mut state = Observer::default();
        for closed in [
            json!({"method":"thread/closed","params":{"threadId":id}}),
            json!({"method":"thread/status/changed","params":{"threadId":id,"status":{"type":"notLoaded"}}}),
        ] {
            state.server(&json!({"method":"thread/started","params":{"thread":{"id":id}}}));
            assert_eq!(state.snapshot.sessions.len(), 1);
            state.server(&closed);
            assert!(state.snapshot.sessions.is_empty());
        }
    }
    #[test]
    fn stopped_or_reused_pid_is_not_an_open_session() {
        let dir = tempfile::tempdir().unwrap();
        let snapshot = Snapshot {
            pid: std::process::id(),
            start_time: process_start(std::process::id()).unwrap() + 1,
            ..Default::default()
        };
        atomic_json(&dir.path().join("hud/instances/stale.json"), &snapshot).unwrap();
        assert!(snapshots(dir.path()).is_empty());
    }
}
