//! Read verified, stopped-profile histories through an isolated official backend.
use crate::{
    managed_files as files,
    profiles::{self, Result},
    shared_chats::ChatLibrary,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    fs,
    io::{BufRead, BufReader, Read},
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportReport {
    pub active_count: usize,
    pub archived_count: usize,
}

pub(crate) fn rollout_id(path: &Path) -> Result<String> {
    let file = fs::File::open(path).map_err(|_| "Could not inspect retained chat history")?;
    let mut line = String::new();
    BufReader::new(file.take(4 * 1024 * 1024))
        .read_line(&mut line)
        .map_err(|_| "Could not inspect retained chat metadata")?;
    let metadata: Value = serde_json::from_str(&line)
        .map_err(|_| "Chat history has an unsupported session header; original data is retained")?;
    let id = metadata["payload"]["id"]
        .as_str()
        .ok_or("Chat session identity is missing")?;
    if metadata["type"] != "session_meta"
        || !Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id)
    {
        return Err(
            "Chat session identity or format is unsupported; original data is retained".into(),
        );
    }
    Ok(id.into())
}

pub(crate) fn rollout_filename(path: &Path, native_id: &str) -> Result<String> {
    if !Uuid::parse_str(native_id).is_ok_and(|id| id.to_string() == native_id) {
        return Err("Invalid native rollout identity".into());
    }
    let file = fs::File::open(path).map_err(|_| "Could not inspect retained rollout timestamp")?;
    let mut line = String::new();
    BufReader::new(file.take(4 * 1024 * 1024))
        .read_line(&mut line)
        .map_err(|_| "Could not inspect retained rollout timestamp")?;
    let header: Value = serde_json::from_str(&line).map_err(|_| "Invalid rollout header")?;
    let timestamp = header["payload"]["timestamp"]
        .as_str()
        .or_else(|| header["timestamp"].as_str());
    let time = match timestamp {
        Some(value) => chrono::DateTime::parse_from_rfc3339(value)
            .map_err(|_| "Unsupported native rollout timestamp; original history is retained")?
            .with_timezone(&chrono::Utc),
        None => chrono::DateTime::from_timestamp(0, 0).ok_or("Invalid rollout timestamp")?,
    };
    Ok(format!(
        "rollout-{}-{native_id}.jsonl",
        time.format("%Y-%m-%dT%H-%M-%S")
    ))
}

pub(crate) fn rollouts(root: &Path, directory: &Path) -> Result<Vec<PathBuf>> {
    files::checked(root, directory, true)?;
    let mut found = Vec::new();
    if !directory.exists() {
        return Ok(found);
    }
    fn walk(root: &Path, directory: &Path, found: &mut Vec<PathBuf>) -> Result<()> {
        for entry in
            fs::read_dir(directory).map_err(|_| "Could not inventory retained chat history")?
        {
            let path = entry
                .map_err(|_| "Could not inventory retained chat entry")?
                .path();
            files::checked(root, &path, false)?;
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| "Could not inspect retained chat entry")?;
            if files::is_link(&metadata) {
                return Err("Linked chat histories need explicit migration coverage".into());
            }
            if metadata.is_dir() {
                walk(root, &path, found)?;
            } else if metadata.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "jsonl")
            {
                // Every transcript is accounted for. Unknown JSONL formats block
                // migration instead of quietly producing an incomplete library.
                rollout_id(&path)?;
                found.push(path);
            } else {
                return Err("An unsupported chat history entry prevents complete migration".into());
            }
        }
        Ok(())
    }
    walk(root, directory, &mut found)?;
    found.sort();
    Ok(found)
}

pub(crate) fn assets(
    snapshot_home: &Path,
    original_home: &Path,
    rollout: &Path,
) -> Result<Vec<(String, PathBuf)>> {
    let mut assets = Vec::new();
    for reference in crate::shared_chats::local_assets(rollout)? {
        let path = if reference.starts_with("file:") {
            url::Url::parse(&reference)
                .map_err(|_| "Chat attachment URL is invalid")?
                .to_file_path()
                .map_err(|_| "Chat attachment URL has no local file")?
        } else {
            PathBuf::from(&reference)
        };
        let source = if path.is_absolute() {
            if let Ok(relative) = path.strip_prefix(original_home) {
                snapshot_home.join(relative)
            } else if path.starts_with(snapshot_home) {
                path
            } else {
                return Err("This chat references an external attachment that is not covered by its verified backup".into());
            }
        } else {
            snapshot_home.join(path)
        };
        let source = files::regular(snapshot_home, &source)?;
        let relative = source
            .strip_prefix(snapshot_home)
            .map_err(|_| "Attachment escaped its snapshot")?;
        let allowed = relative.components().next().is_some_and(|part| {
            matches!(
                part.as_os_str().to_str(),
                Some("attachments" | "assets" | "images" | "shared-assets")
            )
        });
        if !allowed {
            return Err(
                "Only verified attachment directories may supply shared chat assets".into(),
            );
        }
        assets.push((reference, source));
    }
    assets.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(assets)
}

pub fn import_snapshot(
    data_root: &Path,
    binary: &Path,
    snapshot: &crate::migration::SnapshotRecord,
    closed: impl Fn() -> Result<()>,
) -> Result<ImportReport> {
    closed()?;
    let migration = crate::migration::MigrationStore::open(data_root)?;
    migration.verify(snapshot)?;
    let imported = migration.import_home(snapshot)?;
    let source = imported.path().join("codex-home");
    files::root(&source)?;
    let original = data_root
        .join("profiles")
        .join(&snapshot.profile_id)
        .join("codex-home");
    let library = ChatLibrary::open(data_root)?;
    let mut report = ImportReport {
        active_count: 0,
        archived_count: 0,
    };
    for (name, archived) in [("sessions", false), ("archived_sessions", true)] {
        for rollout in rollouts(&source, &source.join(name))? {
            closed()?;
            let native_id = rollout_id(&rollout)?;
            let mappings = assets(&source, &original, &rollout)?;
            // Credentials, account settings, SQLite files and tools never enter
            // this reader. Resuming can modify only the disposable reader copy.
            let metadata = read_thread(data_root, binary, &rollout)?;
            if metadata["id"] != native_id
                || !matches!(
                    metadata["status"]["type"].as_str(),
                    Some("idle" | "notLoaded")
                )
            {
                return Err(
                    "The isolated reader did not verify the complete stopped thread identity"
                        .into(),
                );
            }
            closed()?;
            library.import(
                &snapshot.profile_id,
                &native_id,
                &source,
                &rollout,
                &metadata,
                archived,
                &mappings,
            )?;
            if archived {
                report.archived_count += 1;
            } else {
                report.active_count += 1;
            }
        }
    }
    closed()?;
    migration.verify(snapshot)?;
    Ok(report)
}

pub(crate) fn read_thread(data_root: &Path, binary: &Path, rollout: &Path) -> Result<Value> {
    let native_id = rollout_id(rollout)?;
    let directory = data_root.join("migrations/chat-readers");
    files::directory(data_root, &directory)?;
    let reader = tempfile::tempdir_in(directory)
        .map_err(|_| "Could not allocate isolated history reader")?;
    profiles::set_owner_only_dir(reader.path())?;
    fs::create_dir(reader.path().join("sessions"))
        .map_err(|_| "Could not prepare isolated history reader")?;
    let copy = reader
        .path()
        .join("sessions")
        .join(rollout_filename(rollout, &native_id)?);
    files::copy_tree(rollout, &copy)?;
    profiles::write_codex_config(reader.path())?;
    let mut server = crate::chat_cleanup::LocalServer::start(binary, reader.path())?;
    let resumed = server.request(
        "thread/resume",
        json!({"threadId":native_id,"path":copy,"modelProvider":"openai"}),
    )?;
    let mut metadata = resumed["thread"].clone();
    let file =
        fs::File::open(rollout).map_err(|_| "Could not inspect original provider metadata")?;
    let mut line = String::new();
    BufReader::new(file.take(4 * 1024 * 1024))
        .read_line(&mut line)
        .map_err(|_| "Could not read original provider metadata")?;
    let header: Value =
        serde_json::from_str(&line).map_err(|_| "Invalid original session header")?;
    if let Some(provider) = header["payload"]["model_provider"].as_str() {
        metadata["modelProvider"] = Value::String(provider.to_owned());
    }
    Ok(metadata)
}

pub(crate) fn recover_writer(
    data_root: &Path,
    binary: &Path,
    leases: &crate::shared_chat_leases::ThreadLeases,
    library: &ChatLibrary,
    thread_id: &str,
) -> Result<crate::shared_chats::SharedThread> {
    let lease = leases.recover_stopped(thread_id)?;
    let saved = library.get(thread_id).ok();
    if saved.is_none() && library.contains_id(thread_id)? {
        return Err("A deleted shared chat cannot be recovered automatically. Its retained source and backups are kept".into());
    }
    let native_id = saved
        .as_ref()
        .map(|thread| thread.native_id.as_str())
        .unwrap_or(thread_id);
    let home = &lease.owner.session_home;
    let mut candidates = Vec::new();
    for name in ["sessions", "archived_sessions"] {
        for rollout in rollouts(home, &home.join(name))? {
            if rollout_id(&rollout)? == native_id {
                candidates.push(rollout);
            }
        }
    }
    if candidates.len() != 1 {
        return Err("Stopped writer recovery needs one unambiguous retained rollout; all copies and ownership have been kept".into());
    }
    let rollout = &candidates[0];
    let metadata = read_thread(data_root, binary, rollout)?;
    let mappings = assets(home, home, rollout)?;
    let committed = if saved.is_some() {
        library.publish_stopped(leases, &lease, rollout, &metadata, &mappings)?
    } else {
        let profile = lease.owner.profile_id.as_deref().ok_or("The stopped new chat has no verified source profile; its private history remains retained")?;
        library.import_owned(leases, &lease, profile, rollout, &metadata, &mappings, true)?
    };
    leases.release(&lease)?;
    Ok(committed)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_inventory_and_asset_rebasing_do_not_read_original_auth() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("snapshot");
        fs::create_dir_all(home.join("sessions/nested")).unwrap();
        fs::create_dir(home.join("attachments")).unwrap();
        let id = Uuid::new_v4().to_string();
        let original = temp.path().join("original");
        let path = home.join("sessions/nested/rollout.jsonl");
        let original_asset = original.join("attachments/image.png");
        fs::write(home.join("attachments/image.png"), "full image").unwrap();
        fs::write(&path, format!("{}\n{}\n", json!({"type":"session_meta","payload":{"id":id}}), json!({"type":"response_item","payload":{"type":"localImage","path":original_asset}}))).unwrap();
        assert_eq!(
            rollouts(&home, &home.join("sessions")).unwrap(),
            vec![path.clone()]
        );
        assert_eq!(
            assets(&home, &original, &path).unwrap()[0].1,
            home.join("attachments/image.png")
        );
        fs::write(&path, format!("{}\n{}\n", json!({"type":"session_meta","payload":{"id":id}}), json!({"type":"response_item","payload":{"type":"localImage","path":original.join("auth.json")}}))).unwrap();
        assert!(assets(&home, &original, &path).is_err());
    }
    #[test]
    fn unsupported_transcript_is_not_silently_skipped() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("sessions")).unwrap();
        fs::write(
            temp.path().join("sessions/unknown.jsonl"),
            "{\"type\":\"unknown\"}\n",
        )
        .unwrap();
        assert!(rollouts(temp.path(), &temp.path().join("sessions")).is_err());
    }
}
