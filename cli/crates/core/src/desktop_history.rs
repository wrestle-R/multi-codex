//! Additive import of regular Codex Desktop's local history into a stopped profile.
use crate::{managed_files as files, profiles::Result, shared_chat_import::rollouts};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::{json, Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, BufReader, Read},
    path::{Path, PathBuf},
};

const STATE: &str = ".codex-global-state.json";
const SESSION_INDEX: &str = "session_index.jsonl";

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub chats: usize,
    pub projects: usize,
}

fn inventory(home: &Path) -> Result<BTreeMap<String, PathBuf>> {
    let mut by_id = BTreeMap::new();
    for directory in ["sessions", "archived_sessions"] {
        for path in rollouts(home, &home.join(directory))? {
            let id = crate::shared_chat_import::rollout_id(&path)?;
            if by_id.insert(id, path).is_some() {
                return Err("Duplicate local chat IDs found. Close Codex and check its history before importing.".into());
            }
        }
    }
    Ok(by_id)
}

fn copy_transcript(source: &Path, target_home: &Path, target: &Path) -> Result<()> {
    let parent = target.parent().ok_or("Invalid imported chat path")?;
    files::directory(target_home, parent)?;
    files::checked(target_home, target, true)?;
    let mut snapshot = tempfile::NamedTempFile::new_in(target_home)
        .map_err(|_| "Could not stage the imported chat")?;
    crate::file_security::protect(snapshot.path(), false)?;
    io::copy(
        &mut fs::File::open(source).map_err(|_| "Could not read the original chat")?,
        &mut snapshot,
    )
    .map_err(|_| "Could not copy the original chat")?;
    snapshot
        .as_file()
        .sync_all()
        .map_err(|_| "Could not save the imported chat")?;
    if files::digest(source)? != files::digest(snapshot.path())? {
        return Err(
            "A regular Codex chat changed during import. Close regular Codex and retry.".into(),
        );
    }
    snapshot
        .persist_noclobber(target)
        .map_err(|_| "Could not add the copied chat without overwriting a file")?;
    crate::file_security::sync_directory(parent)
}

fn bounded_file(home: &Path, name: &str) -> Result<Option<Vec<u8>>> {
    const LIMIT: u64 = 64 * 1024 * 1024;
    let path = home.join(name);
    files::checked(home, &path, true)?;
    let handle = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err("Could not read Desktop history metadata".into()),
    };
    if !handle
        .metadata()
        .is_ok_and(|info| info.is_file() && info.len() <= LIMIT)
    {
        return Err("Desktop history metadata has an unsupported size or format".into());
    }
    let mut bytes = Vec::new();
    BufReader::new(handle)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read Desktop history metadata")?;
    if bytes.len() as u64 > LIMIT {
        return Err(
            "Desktop history metadata changed during import. Close regular Codex and retry.".into(),
        );
    }
    Ok(Some(bytes))
}

fn desktop_state(home: &Path) -> Result<Map<String, Value>> {
    bounded_file(home, STATE)?
        .map(|bytes| serde_json::from_slice(&bytes).map_err(|_| "Invalid Desktop project state"))
        .transpose()
        .map(|state| state.unwrap_or_default())
        .map_err(Into::into)
}

fn object<'a>(state: &'a mut Map<String, Value>, key: &str) -> Result<&'a mut Map<String, Value>> {
    state
        .entry(key)
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| "Unsupported profile Desktop project state".into())
}

fn append_unique(
    state: &mut Map<String, Value>,
    key: &str,
    values: impl IntoIterator<Item = Value>,
) -> Result<()> {
    let target = state
        .entry(key)
        .or_insert_with(|| json!([]))
        .as_array_mut()
        .ok_or("Unsupported profile Desktop project order")?;
    for value in values {
        if !target.contains(&value) {
            target.push(value);
        }
    }
    Ok(())
}

// Newer desktop builds also keep their project catalog in the native state DB.
// Read only the project records and assignments. The database stays in its original home.
fn read_native_projects(home: &Path, state: &mut Map<String, Value>) -> Result<()> {
    let path = home.join("state_5.sqlite");
    files::checked(home, &path, true)?;
    if !path.exists() {
        return Ok(());
    }
    files::regular(home, &path)?;
    let error = |_| "Could not read regular Codex's local project catalog";
    let mut source =
        Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(error)?;
    let snapshot = source.transaction().map_err(error)?;
    let has_projects = snapshot
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_schema WHERE type='table' AND name='project_roots')",
            [],
            |row| row.get::<_, bool>(0),
        )
        .map_err(error)?;
    if !has_projects {
        return Ok(());
    }

    let id_map = state
        .get("app-server-project-id-by-legacy-project-id-by-host")
        .and_then(|value| value.get(format!("local:{}", home.display())))
        .and_then(Value::as_object)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(id, value)| Some((value.as_str()?.to_owned(), id.clone())))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut ids = BTreeMap::new();
    let mut projects = snapshot
        .prepare("SELECT id, name, created_at_ms, updated_at_ms FROM projects ORDER BY position")
        .map_err(error)?;
    let mut roots = snapshot
        .prepare("SELECT path FROM project_roots WHERE project_id=?1 ORDER BY position")
        .map_err(error)?;
    let projects = projects
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(error)?;
    for row in projects {
        let (id, name, created_at, updated_at) = row.map_err(error)?;
        let legacy_id = id_map.get(&id).cloned().unwrap_or_else(|| id.clone());
        let paths = roots
            .query_map([&id], |row| row.get::<_, String>(0))
            .map_err(error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(error)?;
        object(state, "local-projects")?.entry(&legacy_id).or_insert_with(|| json!({
            "id":legacy_id, "name":name, "rootPaths":paths, "createdAt":created_at, "updatedAt":updated_at,
        }));
        append_unique(state, "project-order", [json!(legacy_id)])?;
        ids.insert(id, legacy_id);
    }
    if ids.is_empty() {
        return Ok(());
    }
    let mut threads = snapshot
        .prepare("SELECT id, project_id FROM threads WHERE project_id IS NOT NULL")
        .map_err(error)?;
    let threads = threads
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(error)?;
    let assignments = object(state, "thread-project-assignments")?;
    for row in threads {
        let (id, project) = row.map_err(error)?;
        if let Some(project_id) = ids.get(&project) {
            assignments
                .entry(id)
                .or_insert_with(|| json!({"projectKind":"local","projectId":project_id}));
        }
    }
    Ok(())
}

fn merge_projects(
    source: &Map<String, Value>,
    target: &mut Map<String, Value>,
    copied_ids: &BTreeSet<String>,
) -> Result<(usize, BTreeSet<String>)> {
    let mut imported = BTreeSet::new();
    let mut count = 0;
    if let Some(projects) = source.get("local-projects").and_then(Value::as_object) {
        let target_projects = object(target, "local-projects")?;
        for (id, project) in projects {
            let (Some(name), Some(roots)) =
                (project["name"].as_str(), project["rootPaths"].as_array())
            else {
                continue;
            };
            if project["id"] != *id
                || !roots.iter().all(|root| {
                    root.as_str()
                        .is_some_and(|root| Path::new(root).is_absolute())
                })
            {
                continue;
            }
            let record = json!({
                "id": id, "name": name, "rootPaths": roots,
                "createdAt": project["createdAt"].as_i64().unwrap_or_default(),
                "updatedAt": project["updatedAt"].as_i64().unwrap_or_default()
            });
            match target_projects.get(id) {
                Some(existing) if existing != &record => {}
                Some(_) => {
                    imported.insert(id.as_str());
                }
                None => {
                    target_projects.insert(id.clone(), record);
                    imported.insert(id.as_str());
                    count += 1;
                }
            }
        }
    }
    let values = |key| {
        source
            .get(key)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
    };
    let mut order = Vec::new();
    for id in values("project-order")
        .filter_map(Value::as_str)
        .chain(imported.iter().copied())
    {
        if imported.contains(id) {
            order.push(json!(id));
        }
    }
    if !order.is_empty() {
        append_unique(target, "project-order", order)?;
    }
    let roots = values("electron-saved-workspace-roots")
        .filter(|value| {
            value
                .as_str()
                .is_some_and(|path| Path::new(path).is_absolute())
        })
        .cloned()
        .collect::<Vec<_>>();
    if !roots.is_empty() {
        append_unique(target, "electron-saved-workspace-roots", roots)?;
    }

    if let Some(assignments) = source
        .get("thread-project-assignments")
        .and_then(Value::as_object)
    {
        let target_assignments = object(target, "thread-project-assignments")?;
        for (thread, assignment) in assignments {
            if copied_ids.contains(thread)
                && assignment["projectKind"] == "local"
                && assignment["projectId"]
                    .as_str()
                    .is_some_and(|id| imported.contains(id))
            {
                let mut value = json!({"projectKind": "local", "projectId": assignment["projectId"], "pendingCoreUpdate": false});
                if let Some(cwd) = assignment["cwd"]
                    .as_str()
                    .filter(|path| Path::new(path).is_absolute())
                {
                    value["cwd"] = json!(cwd);
                }
                target_assignments.entry(thread).or_insert(value);
            }
        }
    }
    let projectless = values("projectless-thread-ids")
        .filter(|value| value.as_str().is_some_and(|id| copied_ids.contains(id)))
        .cloned()
        .collect::<Vec<_>>();
    if !projectless.is_empty() {
        append_unique(target, "projectless-thread-ids", projectless)?;
    }
    Ok((count, imported.into_iter().map(str::to_owned).collect()))
}

fn index_entries(bytes: &[u8]) -> impl Iterator<Item = Value> + '_ {
    bytes
        .split(|byte| *byte == b'\n')
        .filter_map(|line| serde_json::from_slice(line).ok())
}

fn merge_titles(source: &Path, target: &Path, copied_ids: &BTreeSet<String>) -> Result<()> {
    let Some(source_index) = bounded_file(source, SESSION_INDEX)? else {
        return Ok(());
    };
    let mut target_index = bounded_file(target, SESSION_INDEX)?.unwrap_or_default();
    let existing: BTreeSet<_> = index_entries(&target_index)
        .filter_map(|entry| entry["id"].as_str().map(str::to_owned))
        .collect();
    let mut by_id = BTreeMap::new();
    for value in index_entries(&source_index) {
        if let (Some(id), Some(title), Some(updated_at)) = (
            value["id"].as_str(),
            value["thread_name"].as_str(),
            value["updated_at"].as_str(),
        ) {
            if copied_ids.contains(id) && !existing.contains(id) {
                by_id.insert(
                    id.to_owned(),
                    json!({"id":id,"thread_name":title,"updated_at":updated_at}),
                );
            }
        }
    }
    if by_id.is_empty() {
        return Ok(());
    }
    if !target_index.is_empty() && !target_index.ends_with(b"\n") {
        target_index.push(b'\n');
    }
    for value in by_id.into_values() {
        serde_json::to_writer(&mut target_index, &value)
            .map_err(|_| "Could not encode imported chat names")?;
        target_index.push(b'\n');
    }
    crate::profiles::write_private_file(&target.join(SESSION_INDEX), &target_index)
}

pub fn import(source: &Path, target: &Path, backend: Option<&Path>) -> Result<ImportReport> {
    match fs::symlink_metadata(source) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(ImportReport::default()),
        _ => files::root(source)?,
    }
    if source == target {
        return Err("The regular Codex home and profile home must be separate".into());
    }
    let original = inventory(source)?;
    let managed = inventory(target)?;
    let mut source_state = desktop_state(source)?;
    read_native_projects(source, &mut source_state)?;
    let mut target_state = desktop_state(target)?;
    let mut copies = BTreeMap::new();
    let mut report = ImportReport::default();
    let mut copied_ids = BTreeSet::new();
    for (id, path) in original {
        let relative = path
            .strip_prefix(source)
            .map_err(|_| "Invalid source transcript path")?;
        let destination = target.join(relative);
        if let Some(existing) = managed.get(&id) {
            // A previous attempt may have saved the transcript before indexing it.
            if *existing == destination && files::digest(existing)? == files::digest(&path)? {
                copied_ids.insert(id);
            }
        } else {
            copied_ids.insert(id.clone());
            copies.insert(id, (path, destination));
        }
    }
    let original_state = target_state.clone();
    let (project_count, project_ids) =
        merge_projects(&source_state, &mut target_state, &copied_ids)?;
    report.projects = project_count;
    for (source, destination) in copies.values() {
        copy_transcript(source, target, destination)?;
    }
    if target_state != original_state {
        let bytes = serde_json::to_vec(&target_state)
            .map_err(|_| "Could not encode imported Desktop projects")?;
        crate::profiles::write_private_file(&target.join(STATE), &bytes)?;
    }
    merge_titles(source, target, &copied_ids)?;
    if let Some(backend) = backend.filter(|_| !copied_ids.is_empty() || !project_ids.is_empty()) {
        let mut server = crate::chat_cleanup::LocalServer::start(backend, target)?;
        let target_state = Value::Object(target_state);
        let mut native_projects = BTreeMap::new();
        for id in project_ids {
            let project = &target_state["local-projects"][&id];
            let roots = project["rootPaths"]
                .as_array()
                .ok_or("Invalid project roots")?
                .iter()
                .map(|path| json!({"path":path}))
                .collect::<Vec<_>>();
            let response = server.request(
                "project/import",
                json!({
                    "idempotencyKey": id, "name": project["name"], "roots": roots,
                    "metadata": {"codex.localProjectId": id},
                }),
            )?;
            let native_id = response["project"]["id"]
                .as_str()
                .ok_or("Codex returned no imported project ID")?;
            native_projects.insert(id, native_id.to_owned());
        }
        for id in copied_ids {
            // Read repairs the native index without resuming the conversation.
            let result =
                server.request("thread/read", json!({"threadId":id,"includeTurns":false}))?;
            if result["thread"]["id"] != id {
                return Err(
                    "Codex could not index a copied chat. The original remains in regular Codex."
                        .into(),
                );
            }
            let assignment = &target_state["thread-project-assignments"][&id];
            if result["thread"]["projectId"].is_null() && assignment["projectKind"] == "local" {
                if let Some(project_id) = assignment["projectId"]
                    .as_str()
                    .and_then(|id| native_projects.get(id))
                {
                    server.request(
                        "thread/metadata/update",
                        json!({"threadId":id,"projectId":project_id}),
                    )?;
                }
            }
        }
    }
    report.chats = copies.len();
    Ok(report)
}
