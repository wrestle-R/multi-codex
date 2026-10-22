//! Delete local threads through Codex, which also maintains its history indexes.
use crate::profiles::Result;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const SOURCES: &[&str] = &[
    "cli",
    "vscode",
    "exec",
    "appServer",
    "subAgent",
    "subAgentReview",
    "subAgentCompact",
    "subAgentThreadSpawn",
    "subAgentOther",
    "unknown",
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum ChatScope {
    All,
    OlderThan30Days,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatPreview {
    pub token: String,
    pub count: usize,
    pub transcript_bytes: u64,
    pub protected_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatCleanupResult {
    pub deleted_count: usize,
    pub transcript_bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct ThreadRecord {
    id: String,
    parent: Option<String>,
    last_active: i64,
    path: Option<PathBuf>,
    bytes: u64,
    modified: Option<SystemTime>,
}

pub struct ChatPlan {
    pub profile_id: String,
    pub created: Instant,
    records: Vec<ThreadRecord>,
    selected: Vec<ThreadRecord>,
    pub preview: ChatPreview,
}

impl ChatPlan {
    pub fn expired(&self) -> bool {
        self.created.elapsed() > Duration::from_secs(600)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedPlan {
    schema: u32,
    created_at: i64,
    profile_id: String,
    records: Vec<ThreadRecord>,
    selected: Vec<ThreadRecord>,
    preview: ChatPreview,
}
impl ChatPlan {
    pub fn save(&self, path: &Path) -> Result<()> {
        let saved = SavedPlan {
            schema: 1,
            created_at: chrono::Utc::now().timestamp(),
            profile_id: self.profile_id.clone(),
            records: self.records.clone(),
            selected: self.selected.clone(),
            preview: self.preview.clone(),
        };
        crate::profiles::write_private_file(
            path,
            &serde_json::to_vec(&saved).map_err(|_| "Cannot save deletion preview")?,
        )
    }
    pub fn load(path: &Path) -> Result<Self> {
        let bytes =
            fs::read(path).map_err(|_| "Check the deletion preview again before clearing chats")?;
        if bytes.len() > 16 * 1024 * 1024 {
            return Err("Deletion preview is too large".into());
        }
        let saved: SavedPlan =
            serde_json::from_slice(&bytes).map_err(|_| "Invalid deletion preview")?;
        let age = chrono::Utc::now().timestamp() - saved.created_at;
        if saved.schema != 1 || !(0..=600).contains(&age) {
            return Err("Deletion preview expired. Check it again".into());
        }
        Ok(Self {
            profile_id: saved.profile_id,
            created: Instant::now() - Duration::from_secs(age as u64),
            records: saved.records,
            selected: saved.selected,
            preview: saved.preview,
        })
    }
}

pub fn preview(binary: &Path, home: &Path, profile_id: &str, scope: ChatScope) -> Result<ChatPlan> {
    let mut server = LocalServer::start(binary, home)?;
    let records = server.threads(home)?;
    let cutoff = chrono::Utc::now().timestamp() - 30 * 24 * 60 * 60;
    let (selected, protected_count) = select_threads(&records, scope, cutoff)?;
    let ids: HashSet<_> = selected.iter().map(|record| record.id.as_str()).collect();
    prune_history(home, &ids, true)?;
    let preview = ChatPreview {
        token: uuid::Uuid::new_v4().to_string(),
        count: selected.len(),
        transcript_bytes: selected.iter().map(|record| record.bytes).sum(),
        protected_count,
    };
    Ok(ChatPlan {
        profile_id: profile_id.into(),
        created: Instant::now(),
        records,
        selected,
        preview,
    })
}

pub fn clear(binary: &Path, home: &Path, plan: ChatPlan) -> Result<ChatCleanupResult> {
    let mut server = LocalServer::start(binary, home)?;
    // A preview never silently expands to include chats added or changed later.
    if server.threads(home)? != plan.records {
        return Err("Chat history changed. Check the deletion preview again.".into());
    }
    let mut deleted_count = 0;
    let mut transcript_bytes = 0;
    let mut failure = None;
    for record in &plan.selected {
        if let Some(path) = &record.path {
            if transcript_stamp(home, path).ok() != Some((record.bytes, record.modified)) {
                failure =
                    Some("Chat history changed. Check the deletion preview again.".to_string());
                break;
            }
        }
        if let Err(error) = server.request("thread/delete", json!({"threadId": record.id})) {
            failure = Some(error);
            break;
        }
        deleted_count += 1;
        transcript_bytes += record.bytes;
    }
    // Stop Codex before atomically pruning CLI prompt history and its title index.
    drop(server);
    let ids: HashSet<_> = plan
        .selected
        .iter()
        .take(deleted_count)
        .map(|record| record.id.as_str())
        .collect();
    prune_history(home, &ids, false).map_err(|_| {
        format!(
            "Deleted {deleted_count} chats, but could not finish removing their prompt history."
        )
    })?;
    if let Some(error) = failure {
        return Err(format!(
            "Deleted {deleted_count} of {} chats. {error}",
            plan.selected.len()
        ));
    }
    Ok(ChatCleanupResult {
        deleted_count,
        transcript_bytes,
    })
}

fn prune_history(home: &Path, ids: &HashSet<&str>, check_only: bool) -> Result<()> {
    for (name, id_key) in [
        ("history.jsonl", "session_id"),
        ("session_index.jsonl", "id"),
    ] {
        let path = home.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            _ => return Err("Invalid local prompt history file.".into()),
        }
        let source = fs::File::open(&path).map_err(|_| "Could not read prompt history.")?;
        let mut kept = Vec::new();
        let mut changed = false;
        for line in BufReader::new(source).lines() {
            let line = line.map_err(|_| "Could not read prompt history.")?;
            let value: Value =
                serde_json::from_str(&line).map_err(|_| "Invalid local prompt history record.")?;
            if value[id_key].as_str().is_some_and(|id| ids.contains(id)) {
                changed = true;
            } else {
                kept.extend_from_slice(line.as_bytes());
                kept.push(b'\n');
            }
        }
        if !changed || check_only {
            continue;
        }
        let mut temporary = tempfile::NamedTempFile::new_in(home)
            .map_err(|_| "Could not update prompt history.")?;
        temporary
            .as_file_mut()
            .write_all(&kept)
            .map_err(|_| "Could not update prompt history.")?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|_| "Could not update prompt history.")?;
        crate::file_security::protect(temporary.path(), false)?;
        temporary
            .persist(&path)
            .map_err(|_| "Could not replace prompt history.")?;
    }
    Ok(())
}

fn select_threads(
    records: &[ThreadRecord],
    scope: ChatScope,
    cutoff: i64,
) -> Result<(Vec<ThreadRecord>, usize)> {
    let by_id: BTreeMap<_, _> = records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();
    let mut protected = HashSet::new();
    if scope == ChatScope::OlderThan30Days {
        for record in records
            .iter()
            .filter(|record| record.last_active <= 0 || record.last_active >= cutoff)
        {
            let mut parent = record.parent.as_deref();
            let mut seen = HashSet::new();
            while let Some(id) = parent {
                if !seen.insert(id) {
                    return Err(
                        "Codex returned an invalid chat family. Cleanup was stopped.".into(),
                    );
                }
                protected.insert(id);
                parent = by_id.get(id).and_then(|record| record.parent.as_deref());
            }
        }
    }
    let eligible = |record: &&ThreadRecord| {
        scope == ChatScope::All || (record.last_active > 0 && record.last_active < cutoff)
    };
    let protected_count = records
        .iter()
        .filter(eligible)
        .filter(|record| protected.contains(record.id.as_str()))
        .count();
    let mut selected: Vec<_> = records
        .iter()
        .filter(eligible)
        .filter(|record| !protected.contains(record.id.as_str()))
        .cloned()
        .collect();
    // Delete children first; thread/delete cascades, so the parent must be last.
    let mut depths = BTreeMap::new();
    for record in &selected {
        let mut seen = HashSet::from([record.id.as_str()]);
        let mut parent = record.parent.as_deref();
        let mut depth = 0;
        while let Some(id) = parent {
            if !seen.insert(id) {
                return Err("Codex returned an invalid chat family. Cleanup was stopped.".into());
            }
            depth += 1;
            parent = by_id.get(id).and_then(|record| record.parent.as_deref());
        }
        depths.insert(record.id.clone(), depth);
    }
    selected.sort_by_key(|record| (std::cmp::Reverse(depths[&record.id]), record.id.clone()));
    Ok((selected, protected_count))
}

fn transcript_stamp(home: &Path, path: &Path) -> Result<(u64, Option<SystemTime>)> {
    // Mac launches use a private /tmp alias for long account paths. Accept that
    // outer alias only when it resolves into this same account; interior links
    // in its actual session directory are still refused.
    let supplied_home = home;
    let canonical_home =
        fs::canonicalize(home).map_err(|_| "Could not resolve the account's chat directory.")?;
    let home = canonical_home.as_path();
    let resolved;
    let path = if path.starts_with(supplied_home) {
        resolved = home.join(
            path.strip_prefix(supplied_home)
                .map_err(|_| "Invalid chat transcript path.")?,
        );
        resolved.as_path()
    } else if path.starts_with(home) {
        path
    } else {
        let parent = path.parent().ok_or("Invalid chat transcript path.")?;
        let name = path.file_name().ok_or("Invalid chat transcript path.")?;
        resolved = fs::canonicalize(parent)
            .map_err(|_| "Could not resolve the chat transcript.")?
            .join(name);
        resolved.as_path()
    };
    let relative = path
        .strip_prefix(home)
        .map_err(|_| "Chat transcript is outside this profile. Cleanup was stopped.")?;
    let mut parts = relative.components();
    if !matches!(parts.next(), Some(std::path::Component::Normal(part)) if part == "sessions" || part == "archived_sessions")
    {
        return Err(
            "Chat transcript is outside this profile's session folders. Cleanup was stopped."
                .into(),
        );
    }
    let mut current = home.to_path_buf();
    for part in relative.components() {
        if !matches!(part, std::path::Component::Normal(_)) {
            return Err("Invalid chat transcript path.".into());
        }
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err("Linked chat transcripts cannot be deleted.".into())
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok((0, None)),
            Err(_) => return Err("Could not inspect the chat transcript.".into()),
        }
    }
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "Could not inspect the chat transcript.")?;
    if !metadata.is_file() {
        return Err("Chat transcript is not a regular file.".into());
    }
    Ok((
        metadata.len(),
        Some(
            metadata
                .modified()
                .map_err(|_| "Could not read chat activity time.")?,
        ),
    ))
}

pub struct LocalServer {
    child: Child,
    messages: Receiver<Value>,
    next_id: u64,
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl LocalServer {
    pub fn start(binary: &Path, home: &Path) -> Result<Self> {
        let mut command = Command::new(binary);
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy();
            if name.starts_with("CODEX_")
                || name.starts_with("OPENAI_")
                || name.starts_with("VSCODE_")
            {
                command.env_remove(key);
            }
        }
        let mut child = crate::process::spawn(
            command
                .args(["app-server", "--stdio", "-c", "analytics.enabled=false"])
                .env_remove("NODE_OPTIONS")
                .env_remove("ELECTRON_RUN_AS_NODE")
                .env("CODEX_HOME", home)
                .env("CODEX_SQLITE_HOME", home)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::null()),
        )
        .map_err(|_| "Codex CLI is required to manage local chats.")?;
        let stdout = child
            .stdout
            .take()
            .ok_or("Could not read the local chat service.")?;
        let (sender, messages) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                if let Ok(value) = serde_json::from_str(&line) {
                    if sender.send(value).is_err() {
                        break;
                    }
                }
            }
        });
        let mut server = Self {
            child,
            messages,
            next_id: 1,
        };
        server.request("initialize", json!({"clientInfo": {"name": "multi_codex_local_history", "version": env!("CARGO_PKG_VERSION")}, "capabilities":{"experimentalApi":true}}))?;
        server.send(json!({"method":"initialized"}))?;
        Ok(server)
    }

    fn send(&mut self, message: Value) -> Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .ok_or("The local chat service closed.")?;
        serde_json::to_writer(&mut *stdin, &message)
            .map_err(|_| "Could not send the local chat request.")?;
        stdin
            .write_all(b"\n")
            .and_then(|_| stdin.flush())
            .map_err(|_| "Could not send the local chat request.".into())
    }

    pub fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({"id": id, "method": method, "params": params}))?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let message = self
                .messages
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .map_err(|_| "The local chat service timed out or disconnected.")?;
            if message.get("id").and_then(Value::as_u64) != Some(id) {
                continue;
            }
            if message.get("error").is_some() {
                // Never display raw engine errors, which can include private paths or contents.
                return Err(format!("Codex could not complete {method}. Its saved history remains available; check that the installed engine supports this operation."));
            }
            return message
                .get("result")
                .cloned()
                .ok_or_else(|| "Invalid local chat response.".into());
        }
    }

    fn threads(&mut self, home: &Path) -> Result<Vec<ThreadRecord>> {
        let mut records = BTreeMap::new();
        for archived in [false, true] {
            let mut cursor = Value::Null;
            let mut cursors = HashSet::new();
            loop {
                let response = self.request(
                    "thread/list",
                    json!({
                        "limit":100, "cursor":cursor, "archived":archived,
                        "sourceKinds":SOURCES, "modelProviders":[],
                    }),
                )?;
                let data = response["data"]
                    .as_array()
                    .ok_or("Invalid local chat list.")?;
                for thread in data {
                    if thread["ephemeral"].as_bool() == Some(true) {
                        continue;
                    }
                    if !matches!(
                        thread["status"]["type"].as_str(),
                        Some("notLoaded" | "idle")
                    ) {
                        return Err("A chat is active or its status is uncertain. Close this profile's apps before cleanup.".into());
                    }
                    let id = thread["id"]
                        .as_str()
                        .filter(|id| uuid::Uuid::parse_str(id).is_ok())
                        .ok_or("Invalid local chat identifier.")?
                        .to_string();
                    let parent = thread["parentThreadId"].as_str().map(String::from);
                    let path = thread["path"].as_str().map(PathBuf::from);
                    let (bytes, modified) = path
                        .as_ref()
                        .map(|path| transcript_stamp(home, path))
                        .transpose()?
                        .unwrap_or((0, None));
                    let updated = thread["updatedAt"]
                        .as_i64()
                        .ok_or("Missing chat activity time.")?;
                    let file_time = modified
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map(|time| time.as_secs() as i64)
                        .unwrap_or(0);
                    let last_active = updated
                        .max(thread["recencyAt"].as_i64().unwrap_or(0))
                        .max(file_time);
                    let record = ThreadRecord {
                        id: id.clone(),
                        parent,
                        last_active,
                        path,
                        bytes,
                        modified,
                    };
                    if records.insert(id, record).is_some() {
                        return Err("Chat history changed while checking it. Try again.".into());
                    }
                }
                cursor = response
                    .get("nextCursor")
                    .cloned()
                    .ok_or("Missing chat pagination information.")?;
                if cursor.is_null() {
                    break;
                }
                let next = cursor
                    .as_str()
                    .ok_or("Invalid chat pagination information.")?;
                if !cursors.insert(next.to_string()) {
                    return Err("Invalid chat pagination information.".into());
                }
            }
        }
        Ok(records.into_values().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(id: &str, parent: Option<&str>, last_active: i64) -> ThreadRecord {
        ThreadRecord {
            id: id.into(),
            parent: parent.map(String::from),
            last_active,
            path: None,
            bytes: 12,
            modified: None,
        }
    }

    #[test]
    fn age_cleanup_protects_recent_children_and_their_ancestors() {
        let records = vec![
            record("old", None, 10),
            record("child", Some("old"), 20),
            record("recent", Some("child"), 100),
            record("eligible", None, 49),
            record("boundary", None, 50),
        ];
        let (selected, protected) =
            select_threads(&records, ChatScope::OlderThan30Days, 50).unwrap();
        assert_eq!(
            selected
                .iter()
                .map(|record| record.id.as_str())
                .collect::<Vec<_>>(),
            ["eligible"]
        );
        assert_eq!(protected, 2);
        let (all, _) = select_threads(&records, ChatScope::All, 50).unwrap();
        assert!(
            all.iter().position(|record| record.id == "recent")
                < all.iter().position(|record| record.id == "child")
        );
        assert!(
            all.iter().position(|record| record.id == "child")
                < all.iter().position(|record| record.id == "old")
        );
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn invalid_families_and_unknown_activity_fail_safely() {
        assert!(select_threads(
            &[record("a", Some("b"), 10), record("b", Some("a"), 10)],
            ChatScope::All,
            50
        )
        .is_err());
        let (selected, _) = select_threads(
            &[record("unknown", None, 0)],
            ChatScope::OlderThan30Days,
            50,
        )
        .unwrap();
        assert!(selected.is_empty());
    }

    #[test]
    fn transcript_paths_cannot_escape_session_folders() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        fs::create_dir_all(home.join("sessions")).unwrap();
        let transcript = home.join("sessions/chat.jsonl");
        fs::write(&transcript, "fixture").unwrap();
        assert_eq!(transcript_stamp(&home, &transcript).unwrap().0, 7);
        assert!(transcript_stamp(&home, &root.path().join("outside.jsonl")).is_err());
        assert!(transcript_stamp(&home, &home.join("auth.json")).is_err());
        assert!(transcript_stamp(&home, &home.join("sessions/../auth.json")).is_err());
        #[cfg(unix)]
        {
            let link = home.join("sessions/link.jsonl");
            std::os::unix::fs::symlink(&transcript, &link).unwrap();
            assert!(transcript_stamp(&home, &link).is_err());
            let alias = root.path().join("mac-launch-alias");
            std::os::unix::fs::symlink(&home, &alias).unwrap();
            assert_eq!(
                transcript_stamp(&home, &alias.join("sessions/chat.jsonl"))
                    .unwrap()
                    .0,
                7
            );
            assert_eq!(transcript_stamp(&alias, &transcript).unwrap().0, 7);
            assert!(transcript_stamp(&alias, &link).is_err());
        }
    }

    #[test]
    fn prompt_history_cleanup_preserves_other_chats_and_credentials() {
        let home = tempfile::tempdir().unwrap();
        fs::write(home.path().join("history.jsonl"), "{\"session_id\":\"remove\",\"text\":\"old fixture\"}\n{\"session_id\":\"keep\",\"text\":\"new fixture\"}\n").unwrap();
        fs::write(home.path().join("session_index.jsonl"), "{\"id\":\"remove\",\"thread_name\":\"old\"}\n{\"id\":\"keep\",\"thread_name\":\"new\"}\n").unwrap();
        fs::write(home.path().join("auth.json"), "credential sentinel").unwrap();
        prune_history(home.path(), &HashSet::from(["remove"]), false).unwrap();
        for name in ["history.jsonl", "session_index.jsonl"] {
            let value = fs::read_to_string(home.path().join(name)).unwrap();
            assert!(!value.contains("remove"));
            assert!(value.contains("keep"));
        }
        assert_eq!(
            fs::read_to_string(home.path().join("auth.json")).unwrap(),
            "credential sentinel"
        );
    }

    #[cfg(unix)]
    fn fake_engine(root: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt;
        let binary = root.join("engine");
        fs::write(
            &binary,
            r#"#!/usr/bin/env python3
import json,os,sys
from pathlib import Path
home=Path(os.environ['CODEX_HOME'])
store=home/'fixture-threads.json'
for line in sys.stdin:
    request=json.loads(line)
    if 'id' not in request: continue
    method=request['method'];params=request.get('params',{})
    result={}
    if method=='thread/list':
        rows=json.loads(store.read_text())
        result={'data':[r for r in rows if r['archived']==params['archived']], 'nextCursor':None}
    if method=='thread/delete':
        rows=json.loads(store.read_text());identifier=params['threadId']
        for row in rows:
            if row['id']==identifier: Path(row['path']).unlink(missing_ok=True)
        store.write_text(json.dumps([r for r in rows if r['id']!=identifier]))
        with (home/'deleted-fixture-ids').open('a') as log: log.write(identifier+'\n')
    print(json.dumps({'id':request['id'],'result':result}),flush=True)
"#,
        )
        .unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        binary
    }

    #[cfg(unix)]
    fn chat_fixture(home: &Path) -> Vec<PathBuf> {
        let mut threads = Vec::new();
        let mut paths = Vec::new();
        for (folder, archived) in [("sessions", false), ("archived_sessions", true)] {
            fs::create_dir_all(home.join(folder)).unwrap();
            let id = uuid::Uuid::new_v4().to_string();
            let path = home.join(folder).join(format!("rollout-{id}.jsonl"));
            fs::write(&path, "disposable chat fixture").unwrap();
            threads.push(json!({"id":id,"path":path,"updatedAt":1,"archived":archived,"status":{"type":"notLoaded"}}));
            paths.push(path);
        }
        fs::write(
            home.join("fixture-threads.json"),
            serde_json::to_vec(&threads).unwrap(),
        )
        .unwrap();
        fs::write(home.join("auth.json"), "credential sentinel").unwrap();
        fs::write(home.join("config.toml"), "settings sentinel").unwrap();
        paths
    }

    #[cfg(unix)]
    #[test]
    fn native_protocol_deletes_active_and_archived_chats_but_keeps_other_data() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let paths = chat_fixture(&home);
        let binary = fake_engine(root.path());
        let plan = preview(&binary, &home, "profile", ChatScope::All).unwrap();
        assert_eq!(plan.preview.count, 2);
        assert!(paths.iter().all(|path| path.exists()));
        let result = clear(&binary, &home, plan).unwrap();
        assert_eq!(result.deleted_count, 2);
        assert!(!paths.iter().any(|path| path.exists()));
        assert_eq!(
            fs::read_to_string(home.join("auth.json")).unwrap(),
            "credential sentinel"
        );
        assert_eq!(
            fs::read_to_string(home.join("config.toml")).unwrap(),
            "settings sentinel"
        );
    }

    #[cfg(unix)]
    #[test]
    fn changed_preview_never_sends_a_delete_request() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("home");
        let paths = chat_fixture(&home);
        let binary = fake_engine(root.path());
        let plan = preview(&binary, &home, "profile", ChatScope::All).unwrap();
        fs::write(&paths[0], "recent activity after the preview").unwrap();
        assert!(clear(&binary, &home, plan)
            .unwrap_err()
            .contains("history changed"));
        assert!(paths.iter().all(|path| path.exists()));
        assert!(!home.join("deleted-fixture-ids").exists());
    }
}

#[cfg(test)]
mod persistent_preview_tests {
    use super::*;
    #[test]
    fn saved_preview_round_trips_file_stamps_and_refuses_expired_or_unknown_schema() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("preview.json");
        let record = ThreadRecord {
            id: uuid::Uuid::new_v4().to_string(),
            parent: None,
            last_active: 123,
            path: Some(directory.path().join("owned.jsonl")),
            bytes: 9,
            modified: Some(SystemTime::now()),
        };
        let plan = ChatPlan {
            profile_id: uuid::Uuid::new_v4().to_string(),
            created: Instant::now(),
            records: vec![record.clone()],
            selected: vec![record],
            preview: ChatPreview {
                token: uuid::Uuid::new_v4().to_string(),
                count: 1,
                transcript_bytes: 9,
                protected_count: 0,
            },
        };
        plan.save(&path).unwrap();
        let loaded = ChatPlan::load(&path).unwrap();
        assert_eq!(loaded.records, plan.records);
        assert_eq!(loaded.selected, plan.selected);
        assert_eq!(loaded.profile_id, plan.profile_id);
        let mut saved: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        saved["created_at"] = json!(chrono::Utc::now().timestamp() - 601);
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(ChatPlan::load(&path).is_err());
        saved["created_at"] = json!(chrono::Utc::now().timestamp());
        saved["schema"] = json!(99);
        fs::write(&path, serde_json::to_vec(&saved).unwrap()).unwrap();
        assert!(ChatPlan::load(&path).is_err());
    }
}
