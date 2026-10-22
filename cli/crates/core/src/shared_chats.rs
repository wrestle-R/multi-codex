//! Durable full-rollout checkpoints and a MultiCodex-owned catalog, separate from Codex SQLite.
use crate::managed_files as files;
use crate::profiles::{self, Result};
use crate::shared_chat_leases::{ThreadLeases, WriterLease};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::Duration;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetReference {
    pub original_path: String,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Checkpoint {
    pub schema: u32,
    pub native_id: String,
    pub rollout_digest: String,
    pub assets: Vec<AssetReference>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedThread {
    pub id: String,
    pub native_id: String,
    pub source_profile_id: String,
    pub checkpoint: String,
    pub revision: u64,
    pub metadata: Value,
    pub archived: bool,
}

pub struct ChatLibrary {
    data_root: PathBuf,
    root: PathBuf,
}

fn identifier(id: &str) -> Result<()> {
    if !Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id) {
        return Err("Invalid shared chat identifier".into());
    }
    Ok(())
}
fn hash_id(id: &str) -> Result<()> {
    if id.len() != 64
        || !id
            .bytes()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
    {
        return Err("Invalid checkpoint content identifier".into());
    }
    Ok(())
}
fn sql_error(_: rusqlite::Error) -> String {
    "Shared chat catalog operation failed; committed history has been retained".into()
}
fn record_change(connection: &Connection, id: &str, kind: &str) -> Result<()> {
    let metadata: String = connection
        .query_row("SELECT metadata FROM threads WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .map_err(sql_error)?;
    connection
        .execute(
            "INSERT INTO changes(thread,kind,metadata) VALUES(?1,?2,?3)",
            params![id, kind, metadata],
        )
        .map_err(sql_error)?;
    connection
        .execute(
            "DELETE FROM changes WHERE seq <= ?1",
            [connection.last_insert_rowid().saturating_sub(5000)],
        )
        .map_err(sql_error)?;
    Ok(())
}

impl ChatLibrary {
    pub fn open(data_root: &Path) -> Result<Self> {
        files::root(data_root)?;
        let root = data_root.join("shared-chats");
        for path in [
            root.join("checkpoints"),
            root.join("assets"),
            root.join("staging"),
        ] {
            files::directory(data_root, &path)?;
        }
        let library = Self {
            data_root: data_root.into(),
            root,
        };
        let _lock = library.lock()?;
        let connection = library.connection()?;
        let version: u32 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .map_err(sql_error)?;
        if version == 0 {
            connection.execute_batch("BEGIN IMMEDIATE;
                CREATE TABLE threads (
                    id TEXT PRIMARY KEY, native_id TEXT NOT NULL, source_profile TEXT NOT NULL,
                    source_thread TEXT NOT NULL, checkpoint TEXT NOT NULL, revision INTEGER NOT NULL,
                    metadata TEXT NOT NULL, archived INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0,
                    updated_at INTEGER NOT NULL, UNIQUE(source_profile, source_thread));
                CREATE INDEX threads_order ON threads(deleted, archived, updated_at DESC, id);
                PRAGMA user_version=1; COMMIT;").map_err(sql_error)?;
        } else if version != 1 {
            return Err(
                "Shared chat catalog version is unsupported; original histories have been kept"
                    .into(),
            );
        }
        connection.execute_batch("CREATE TABLE IF NOT EXISTS source_imports (profile TEXT NOT NULL, thread TEXT NOT NULL, checkpoint TEXT NOT NULL, PRIMARY KEY(profile,thread));").map_err(sql_error)?;
        connection.execute_batch("CREATE TABLE IF NOT EXISTS changes (seq INTEGER PRIMARY KEY AUTOINCREMENT, thread TEXT NOT NULL, kind TEXT NOT NULL, metadata TEXT NOT NULL);").map_err(sql_error)?;
        crate::file_security::protect(&library.root.join("catalog.sqlite"), false)?;
        Ok(library)
    }

    fn lock(&self) -> Result<crate::file_security::ExclusiveLock> {
        let path = files::checked(&self.data_root, &self.root.join("operation.lock"), true)?;
        let file = crate::file_security::lock_file(&path)?;
        crate::file_security::exclusive_lock_timeout(
            file,
            "Shared chat storage is busy; retry when the current operation finishes",
            std::time::Duration::from_secs(5),
        )
    }

    fn connection(&self) -> Result<Connection> {
        let path = files::checked(&self.data_root, &self.root.join("catalog.sqlite"), true)?;
        for suffix in ["-journal", "-wal", "-shm"] {
            files::checked(
                &self.data_root,
                &self.root.join(format!("catalog.sqlite{suffix}")),
                true,
            )?;
        }
        let connection = Connection::open(path).map_err(sql_error)?;
        connection
            .busy_timeout(Duration::from_secs(5))
            .map_err(sql_error)?;
        connection
            .execute_batch("PRAGMA synchronous=FULL; PRAGMA journal_mode=DELETE;")
            .map_err(sql_error)?;
        Ok(connection)
    }

    fn checkpoint_directory(&self, digest: &str) -> Result<PathBuf> {
        hash_id(digest)?;
        files::checked(
            &self.data_root,
            &self.root.join("checkpoints").join(digest),
            false,
        )
    }

    pub fn checkpoint(&self, digest: &str) -> Result<Checkpoint> {
        let directory = self.checkpoint_directory(digest)?;
        let bytes = fs::read(files::regular(
            &self.data_root,
            &directory.join("manifest.json"),
        )?)
        .map_err(|_| "Cannot read shared checkpoint manifest")?;
        let checkpoint: Checkpoint =
            serde_json::from_slice(&bytes).map_err(|_| "Shared checkpoint manifest is invalid")?;
        if checkpoint.schema != 1 || format!("{:x}", Sha256::digest(&bytes)) != digest {
            return Err("Shared checkpoint identity or schema is invalid".into());
        }
        identifier(&checkpoint.native_id)?;
        if files::digest(&files::regular(
            &self.data_root,
            &directory.join("rollout.jsonl"),
        )?)? != checkpoint.rollout_digest
        {
            return Err(
                "Shared history failed integrity verification; original sources remain available"
                    .into(),
            );
        }
        for asset in &checkpoint.assets {
            hash_id(&asset.digest)?;
            if files::digest(&files::regular(
                &self.data_root,
                &self.root.join("assets").join(&asset.digest),
            )?)? != asset.digest
            {
                return Err(
                    "Shared attachment is missing or has changed; full continuation is blocked"
                        .into(),
                );
            }
        }
        Ok(checkpoint)
    }

    /// Source roots must be a verified stopped-profile snapshot or a private owned backend.
    /// The caller resolves supported asset references before publication; incomplete
    /// asset coverage must never be presented as a full import.
    pub fn import(
        &self,
        profile_id: &str,
        source_thread: &str,
        source_root: &Path,
        rollout: &Path,
        metadata: &Value,
        archived: bool,
        assets: &[(String, PathBuf)],
    ) -> Result<SharedThread> {
        self.import_inner(
            profile_id,
            source_thread,
            source_root,
            rollout,
            metadata,
            archived,
            assets,
            false,
        )
    }
    pub fn import_owned(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        profile_id: &str,
        rollout: &Path,
        metadata: &Value,
        assets: &[(String, PathBuf)],
        stopped: bool,
    ) -> Result<SharedThread> {
        let verify = || {
            if stopped {
                leases.verify_stopped(lease)
            } else {
                leases.verify_writer(lease)
            }
        };
        verify()?;
        let committed = self.import_inner(
            profile_id,
            &lease.owner.thread_id,
            &lease.owner.session_home,
            rollout,
            metadata,
            false,
            assets,
            true,
        )?;
        verify()?;
        Ok(committed)
    }
    fn import_inner(
        &self,
        profile_id: &str,
        source_thread: &str,
        source_root: &Path,
        rollout: &Path,
        metadata: &Value,
        archived: bool,
        assets: &[(String, PathBuf)],
        owned: bool,
    ) -> Result<SharedThread> {
        identifier(profile_id)?;
        identifier(source_thread)?;
        if metadata["id"].as_str() != Some(source_thread) {
            return Err("Native chat metadata does not match the imported history".into());
        }
        let _lock = self.lock()?;
        let mut connection = self.connection()?;
        let existing: Option<(String, String, i64)> = connection.query_row(
            "SELECT id, checkpoint, deleted FROM threads WHERE source_profile=?1 AND source_thread=?2",
            params![profile_id, source_thread], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).optional().map_err(sql_error)?;
        if existing.as_ref().is_some_and(|record| record.2 != 0) {
            return Err("This chat was deleted from the shared library; source scans will not restore it automatically".into());
        }
        let checkpoint = self.store_checkpoint(source_thread, source_root, rollout, assets)?;
        if existing.is_none() {
            let deleted_native: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM threads WHERE native_id=?1 AND deleted=1)",
                    [source_thread],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            if deleted_native {
                return Err("This legacy chat shares an identity with deleted shared history. Its original copy is retained; deliberate restore or conflict resolution is required before importing it".into());
            }
        }
        let prior_source: Option<String> = connection
            .query_row(
                "SELECT checkpoint FROM source_imports WHERE profile=?1 AND thread=?2",
                params![profile_id, source_thread],
                |row| row.get(0),
            )
            .optional()
            .map_err(sql_error)?;
        if let Some((id, current, _)) = &existing {
            if current != &checkpoint {
                if prior_source.as_ref() == Some(&checkpoint) {
                    // Rescanning an unchanged legacy source cannot undo a newer
                    // continuation published by a different saved account.
                    return self.get(id);
                }
                if prior_source.as_ref() != Some(current) {
                    return Err("Both the legacy source and shared chat changed independently. Their full checkpoints have been retained; resolve this conflict before migrating the source".into());
                }
            }
        }
        let id = if let Some((id, _, _)) = &existing {
            id.clone()
        } else {
            let collision: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM threads WHERE id=?1)",
                    [source_thread],
                    |row| row.get(0),
                )
                .map_err(sql_error)?;
            if collision {
                Uuid::new_v4().to_string()
            } else {
                source_thread.into()
            }
        };
        let mut clean_metadata = metadata.clone();
        let _writer_guard = if owned {
            None
        } else {
            Some(ThreadLeases::unowned_guard(&self.data_root, &id)?)
        };
        clean_metadata
            .as_object_mut()
            .ok_or("Invalid native chat metadata")?
            .remove("turns");
        // Live native paths belong to a private source, not the canonical shared catalog.
        clean_metadata.as_object_mut().unwrap().remove("path");
        let metadata_json = serde_json::to_string(&clean_metadata)
            .map_err(|_| "Could not encode shared chat metadata")?;
        let updated = metadata["updatedAt"]
            .as_i64()
            .unwrap_or_else(|| chrono::Utc::now().timestamp());
        let transaction = connection.transaction().map_err(sql_error)?;
        transaction.execute("INSERT INTO threads(id,native_id,source_profile,source_thread,checkpoint,revision,metadata,archived,updated_at)
            VALUES(?1,?2,?3,?2,?4,1,?5,?6,?7)
            ON CONFLICT(source_profile,source_thread) DO UPDATE SET
            revision=threads.revision + CASE WHEN threads.checkpoint!=excluded.checkpoint THEN 1 ELSE 0 END,
            checkpoint=excluded.checkpoint,metadata=excluded.metadata,archived=excluded.archived,updated_at=excluded.updated_at",
            params![id, source_thread, profile_id, checkpoint, metadata_json, archived, updated]).map_err(sql_error)?;
        transaction.execute("INSERT INTO source_imports(profile,thread,checkpoint) VALUES(?1,?2,?3) ON CONFLICT(profile,thread) DO UPDATE SET checkpoint=excluded.checkpoint", params![profile_id, source_thread, checkpoint]).map_err(sql_error)?;
        record_change(
            &transaction,
            &id,
            if archived { "archived" } else { "updated" },
        )?;
        transaction.commit().map_err(sql_error)?;
        self.get(&id)
    }

    fn store_checkpoint(
        &self,
        native_id: &str,
        source_root: &Path,
        rollout: &Path,
        assets: &[(String, PathBuf)],
    ) -> Result<String> {
        let rollout = files::regular(source_root, rollout)?;
        validate_rollout(&rollout, native_id)?;
        let mappings: std::collections::HashSet<_> = assets
            .iter()
            .map(|(original, _)| original.as_str())
            .collect();
        for path in local_assets(&rollout)? {
            if !mappings.contains(path.as_str()) {
                return Err("Chat has an attachment outside complete import coverage. Original history is kept and shared continuation is blocked".into());
            }
        }
        let digest = files::digest(&rollout)?;
        let temporary = tempfile::tempdir_in(self.root.join("staging"))
            .map_err(|_| "Cannot allocate checkpoint staging")?;
        let stage = temporary.path().to_path_buf();
        profiles::set_owner_only_dir(&stage)?;
        files::copy_tree(&rollout, &stage.join("rollout.jsonl"))?;
        if files::digest(&stage.join("rollout.jsonl"))? != digest
            || files::digest(&rollout)? != digest
        {
            return Err(
                "Chat changed during checkpointing; retry after the turn is settled".into(),
            );
        }
        let mut references = Vec::new();
        let mut originals = std::collections::HashSet::new();
        for (original_path, source) in assets {
            if !originals.insert(original_path) {
                return Err("Duplicate attachment mapping in chat checkpoint".into());
            }
            let source = files::regular(source_root, source)?;
            let relative = source
                .strip_prefix(source_root)
                .map_err(|_| "Attachment is outside its private source")?;
            let category = relative
                .components()
                .next()
                .and_then(|part| part.as_os_str().to_str());
            if !matches!(
                category,
                Some("attachments" | "assets" | "images" | "shared-assets")
            ) {
                return Err("Only supported chat attachment directories may enter shared storage; credentials and configuration remain private".into());
            }
            let asset_digest = files::digest(&source)?;
            let target = self.root.join("assets").join(&asset_digest);
            files::checked(&self.data_root, &target, true)?;
            if !target.exists() {
                let temporary = stage.join(format!("asset-{}", Uuid::new_v4()));
                files::copy_tree(&source, &temporary)?;
                if files::digest(&temporary)? != asset_digest
                    || files::digest(&source)? != asset_digest
                {
                    return Err(
                        "Attachment changed while importing; full continuation is blocked".into(),
                    );
                }
                fs::rename(&temporary, &target)
                    .map_err(|_| "Could not publish shared attachment")?;
                crate::file_security::sync_directory(&self.root.join("assets"))?;
            } else if files::digest(&target)? != asset_digest {
                return Err("Previously stored attachment failed verification".into());
            }
            references.push(AssetReference {
                original_path: original_path.clone(),
                digest: asset_digest,
            });
        }
        references.sort_by(|a, b| a.original_path.cmp(&b.original_path));
        let checkpoint = Checkpoint {
            schema: 1,
            native_id: native_id.into(),
            rollout_digest: digest,
            assets: references,
        };
        let manifest =
            serde_json::to_vec(&checkpoint).map_err(|_| "Could not encode shared checkpoint")?;
        let content_id = format!("{:x}", Sha256::digest(&manifest));
        profiles::write_private_file(&stage.join("manifest.json"), &manifest)?;
        let destination = self.root.join("checkpoints").join(&content_id);
        files::checked(&self.data_root, &destination, true)?;
        if !destination.exists() {
            fs::rename(&stage, &destination)
                .map_err(|_| "Could not publish complete chat checkpoint")?;
            crate::file_security::sync_directory(&self.root.join("checkpoints"))?;
        }
        self.checkpoint(&content_id)?;
        Ok(content_id)
    }

    pub fn get(&self, id: &str) -> Result<SharedThread> {
        identifier(id)?;
        let row: (String, String, String, String, i64, String, bool) = self.connection()?.query_row(
            "SELECT id,native_id,source_profile,checkpoint,revision,metadata,archived FROM threads WHERE id=?1 AND deleted=0", [id],
            |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get(4)?,row.get(5)?,row.get(6)?))).optional().map_err(sql_error)?.ok_or("This shared chat was deleted or is missing from the library")?;
        Ok(SharedThread {
            id: row.0,
            native_id: row.1,
            source_profile_id: row.2,
            checkpoint: row.3,
            revision: u64::try_from(row.4).map_err(|_| "Invalid shared chat revision")?,
            metadata: serde_json::from_str(&row.5)
                .map_err(|_| "Stored chat metadata is invalid")?,
            archived: row.6,
        })
    }
    pub fn contains_id(&self, id: &str) -> Result<bool> {
        identifier(id)?;
        self.connection()?
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM threads WHERE id=?1)",
                [id],
                |row| row.get(0),
            )
            .map_err(sql_error)
    }

    pub fn list(&self, archived: bool, offset: u32, limit: u32) -> Result<Vec<SharedThread>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id FROM threads WHERE deleted=0 AND archived=?1 ORDER BY updated_at DESC,id LIMIT ?2 OFFSET ?3").map_err(sql_error)?;
        let ids = statement
            .query_map(params![archived, limit.clamp(1, 100), offset], |row| {
                row.get::<_, String>(0)
            })
            .map_err(sql_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        ids.iter().map(|id| self.get(id)).collect()
    }

    /// A writer may append a new durable revision only through its held kernel lease.
    pub fn publish_writer(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        rollout: &Path,
        metadata: &Value,
        assets: &[(String, PathBuf)],
    ) -> Result<SharedThread> {
        self.publish_owned(leases, lease, rollout, metadata, assets, false)
    }

    pub fn publish_stopped(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        rollout: &Path,
        metadata: &Value,
        assets: &[(String, PathBuf)],
    ) -> Result<SharedThread> {
        self.publish_owned(leases, lease, rollout, metadata, assets, true)
    }

    fn publish_owned(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        rollout: &Path,
        metadata: &Value,
        assets: &[(String, PathBuf)],
        stopped: bool,
    ) -> Result<SharedThread> {
        let verify = || {
            if stopped {
                leases.verify_stopped(lease)
            } else {
                leases.verify_writer(lease)
            }
        };
        verify()?;
        let thread = self.get(&lease.owner.thread_id)?;
        if metadata["id"].as_str() != Some(&thread.native_id) {
            return Err("Updated native history belongs to another chat".into());
        }
        let _lock = self.lock()?;
        let checkpoint = self.store_checkpoint(
            &thread.native_id,
            &lease.owner.session_home,
            rollout,
            assets,
        )?;
        verify()?;
        let mut metadata = metadata.clone();
        let object = metadata
            .as_object_mut()
            .ok_or("Native chat metadata is invalid")?;
        object.remove("turns");
        object.remove("path");
        let native_updated = metadata["updatedAt"].as_i64().unwrap_or(0);
        let updated = if checkpoint != thread.checkpoint {
            native_updated.max(chrono::Utc::now().timestamp())
        } else {
            native_updated.max(thread.metadata["updatedAt"].as_i64().unwrap_or(0))
        };
        metadata["updatedAt"] = Value::from(updated);
        let encoded =
            serde_json::to_string(&metadata).map_err(|_| "Cannot encode updated chat activity")?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(sql_error)?;
        transaction
            .execute(
                "UPDATE threads SET revision=revision + CASE WHEN checkpoint!=?1 THEN 1 ELSE 0 END,
            checkpoint=?1,metadata=?2,updated_at=?3 WHERE id=?4 AND deleted=0",
                params![checkpoint, encoded, updated, thread.id],
            )
            .map_err(sql_error)?;
        record_change(&transaction, &thread.id, "updated")?;
        transaction.commit().map_err(sql_error)?;
        self.get(&thread.id)
    }

    pub fn set_archived(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        archived: bool,
    ) -> Result<()> {
        self.set_archived_verified(leases, lease, archived, false)
    }

    pub fn set_archived_stopped(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        archived: bool,
    ) -> Result<()> {
        self.set_archived_verified(leases, lease, archived, true)
    }

    fn set_archived_verified(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        archived: bool,
        stopped: bool,
    ) -> Result<()> {
        if stopped {
            leases.verify_stopped(lease)?;
        } else {
            leases.verify_writer(lease)?;
        }
        let _lock = self.lock()?;
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(sql_error)?;
        transaction
            .execute(
                "UPDATE threads SET archived=?1 WHERE id=?2 AND deleted=0",
                params![archived, lease.owner.thread_id],
            )
            .map_err(sql_error)?;
        record_change(
            &transaction,
            &lease.owner.thread_id,
            if archived { "archived" } else { "unarchived" },
        )?;
        transaction.commit().map_err(sql_error)?;
        Ok(())
    }

    pub fn tombstone(&self, leases: &ThreadLeases, lease: &WriterLease) -> Result<()> {
        self.tombstone_verified(leases, lease, false)
    }

    pub fn tombstone_stopped(&self, leases: &ThreadLeases, lease: &WriterLease) -> Result<()> {
        self.tombstone_verified(leases, lease, true)
    }

    fn tombstone_verified(
        &self,
        leases: &ThreadLeases,
        lease: &WriterLease,
        stopped: bool,
    ) -> Result<()> {
        if stopped {
            leases.verify_stopped(lease)?;
        } else {
            leases.verify_writer(lease)?;
        }
        let _lock = self.lock()?;
        // Keep original checkpoint and source mapping for recovery and no-resurrection.
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(sql_error)?;
        transaction
            .execute(
                "UPDATE threads SET deleted=1 WHERE id=?1",
                [&lease.owner.thread_id],
            )
            .map_err(sql_error)?;
        record_change(&transaction, &lease.owner.thread_id, "deleted")?;
        transaction.commit().map_err(sql_error)?;
        Ok(())
    }

    pub fn changes(&self, since: Option<i64>) -> Result<Value> {
        let connection = self.connection()?;
        let latest: i64 = connection
            .query_row("SELECT COALESCE(MAX(seq),0) FROM changes", [], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        let Some(since) = since else {
            return Ok(serde_json::json!({"cursor":latest,"data":[],"reset":false}));
        };
        if since < 0 || since > latest {
            return Err("Invalid shared catalog cursor".into());
        }
        let oldest: i64 = connection
            .query_row("SELECT COALESCE(MIN(seq),0) FROM changes", [], |row| {
                row.get(0)
            })
            .map_err(sql_error)?;
        let mut statement = connection
            .prepare(
                "SELECT seq,thread,kind,metadata FROM changes WHERE seq>?1 ORDER BY seq LIMIT 100",
            )
            .map_err(sql_error)?;
        let rows = statement
            .query_map([since], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        let cursor = rows.last().map(|row| row.0).unwrap_or(latest);
        let data: Vec<_> = rows.into_iter().map(|(seq,id,kind,metadata)| Ok(serde_json::json!({"sequence":seq,"threadId":id,"kind":kind,"metadata":serde_json::from_str::<Value>(&metadata).map_err(|_| "Invalid shared catalog event")?}))).collect::<Result<_>>()?;
        Ok(
            serde_json::json!({"cursor":cursor,"data":data,"reset":since < oldest.saturating_sub(1)}),
        )
    }

    pub fn hydrate(&self, id: &str, destination_home: &Path) -> Result<PathBuf> {
        let _lock = self.lock()?;
        let thread = self.get(id)?;
        let checkpoint = self.checkpoint(&thread.checkpoint)?;
        files::checked(&self.data_root, destination_home, false)?;
        let session_root = destination_home.join("sessions/shared-library");
        files::directory(&self.data_root, &session_root)?;
        let checkpoint_root = self.checkpoint_directory(&thread.checkpoint)?;
        let rollout = session_root.join(crate::shared_chat_import::rollout_filename(
            &checkpoint_root.join("rollout.jsonl"),
            &thread.native_id,
        )?);
        if rollout.exists() {
            return Err(
                "Private thread working copy already exists; use a fresh backend for hydration"
                    .into(),
            );
        }
        files::copy_tree(
            &self
                .checkpoint_directory(&thread.checkpoint)?
                .join("rollout.jsonl"),
            &rollout,
        )?;
        let mut replacements = std::collections::HashMap::new();
        for asset in &checkpoint.assets {
            let destination = destination_home.join("shared-assets").join(&asset.digest);
            files::directory(&self.data_root, &destination_home.join("shared-assets"))?;
            if !destination.exists() {
                files::copy_tree(&self.root.join("assets").join(&asset.digest), &destination)?;
            }
            if files::digest(&files::regular(&self.data_root, &destination)?)? != asset.digest {
                return Err("Private attachment hydration failed verification".into());
            }
            replacements.insert(
                asset.original_path.clone(),
                destination
                    .to_str()
                    .ok_or("Attachment path is not valid Unicode")?
                    .to_owned(),
            );
        }
        if !replacements.is_empty() {
            let original =
                fs::read_to_string(&rollout).map_err(|_| "Cannot read hydrated history")?;
            let mut rewritten = String::new();
            for line in original.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                let mut value: Value =
                    serde_json::from_str(line).map_err(|_| "Hydrated history format is invalid")?;
                rewrite_assets(&mut value, &replacements)?;
                rewritten.push_str(
                    &serde_json::to_string(&value).map_err(|_| "Cannot encode hydrated history")?,
                );
                rewritten.push('\n');
            }
            profiles::write_private_file(&rollout, rewritten.as_bytes())?;
            for path in local_assets(&rollout)? {
                let local_path = if path.starts_with("file:") {
                    url::Url::parse(&path)
                        .map_err(|_| "Restored image URL is invalid")?
                        .to_file_path()
                        .map_err(|_| "Restored image URL has no local file")?
                } else {
                    PathBuf::from(&path)
                };
                if !local_path.starts_with(destination_home.join("shared-assets")) {
                    return Err("An attachment could not be restored completely; shared continuation is blocked".into());
                }
            }
        }
        Ok(rollout)
    }

    /// Only unreachable broker objects are collected. Backups and native private
    /// working histories never enter this collector's path boundary.
    pub fn collect_unreachable(&self) -> Result<u64> {
        let _lock = self.lock()?;
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT checkpoint FROM threads WHERE deleted=0 UNION SELECT s.checkpoint FROM source_imports s JOIN threads t ON t.source_profile=s.profile AND t.source_thread=s.thread WHERE t.deleted=0").map_err(sql_error)?;
        let checkpoints = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(sql_error)?
            .collect::<std::result::Result<std::collections::HashSet<_>, _>>()
            .map_err(sql_error)?;
        let mut assets = std::collections::HashSet::new();
        for digest in &checkpoints {
            for asset in self.checkpoint(digest)?.assets {
                assets.insert(asset.digest);
            }
        }
        let mut reclaimed = 0u64;
        for (name, reachable) in [("checkpoints", checkpoints), ("assets", assets)] {
            let root = self.root.join(name);
            for entry in fs::read_dir(&root).map_err(|_| "Cannot inventory unused chat objects")? {
                let path = entry.map_err(|_| "Cannot inventory chat object")?.path();
                let digest = path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or("Invalid chat object name")?;
                hash_id(digest)?;
                if reachable.contains(digest) {
                    continue;
                }
                files::checked(&self.data_root, &path, false)?;
                let metadata =
                    fs::symlink_metadata(&path).map_err(|_| "Cannot inspect unused chat object")?;
                if metadata.is_dir() {
                    let manifest = files::tree_manifest(&path)?;
                    if manifest.iter().any(|(_, kind)| kind.starts_with("link:")) {
                        return Err("Linked broker objects prevent cleanup".into());
                    }
                    let bytes = manifest
                        .iter()
                        .filter(|(_, kind)| kind.starts_with("file:"))
                        .try_fold(0u64, |sum, (relative, _)| {
                            fs::metadata(path.join(relative))
                                .map(|m| sum.saturating_add(m.len()))
                                .map_err(|_| "Cannot measure unused chat object")
                        })?;
                    fs::remove_dir_all(path).map_err(|_| "Cannot clear unused checkpoint")?;
                    reclaimed = reclaimed.saturating_add(bytes);
                } else {
                    files::regular(&self.data_root, &path)?;
                    fs::remove_file(path).map_err(|_| "Cannot clear unused attachment")?;
                    reclaimed = reclaimed.saturating_add(metadata.len());
                }
            }
            crate::file_security::sync_directory(&root)?;
        }
        Ok(reclaimed)
    }

    pub fn cleanup_rows(&self) -> Result<Vec<(String, String, i64, Value, bool)>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT id,checkpoint,revision,metadata,archived FROM threads WHERE deleted=0 ORDER BY id").map_err(sql_error)?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, bool>(4)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<std::result::Result<Vec<_>, _>>()
            .map_err(sql_error)?;
        rows.into_iter()
            .map(|(id, checkpoint, revision, metadata, archived)| {
                Ok((
                    id,
                    checkpoint,
                    revision,
                    serde_json::from_str(&metadata)
                        .map_err(|_| "Invalid shared cleanup metadata")?,
                    archived,
                ))
            })
            .collect()
    }
    pub fn cleanup_tombstones(
        &self,
        expected: &[(String, String, i64, Value, bool)],
        selected: &[String],
    ) -> Result<()> {
        let _lock = self.lock()?;
        if self.cleanup_rows()? != expected {
            return Err("Shared chats changed. Check the preview again".into());
        }
        let mut connection = self.connection()?;
        let transaction = connection.transaction().map_err(sql_error)?;
        for id in selected {
            identifier(id)?;
            transaction
                .execute(
                    "UPDATE threads SET deleted=1 WHERE id=?1 AND deleted=0",
                    [id],
                )
                .map_err(sql_error)?;
            record_change(&transaction, id, "deleted")?;
        }
        transaction.commit().map_err(sql_error)?;
        Ok(())
    }
}

fn asset_keys(value: &Value) -> &'static [&'static str] {
    match value["type"].as_str() {
        Some("localImage" | "local_image" | "localAudio" | "local_audio") => &["path"],
        Some("input_image") => &["image_url"],
        Some("image" | "audio") => &["url"],
        _ => &[],
    }
}

pub fn local_assets(path: &Path) -> Result<std::collections::HashSet<String>> {
    fn collect(value: &Value, paths: &mut std::collections::HashSet<String>) -> Result<()> {
        if matches!(
            value["type"].as_str(),
            Some(
                "image"
                    | "input_image"
                    | "audio"
                    | "input_audio"
                    | "file"
                    | "input_file"
                    | "document"
            )
        ) && (value.get("fileId").is_some() || value.get("file_id").is_some())
        {
            return Err(
                "Account-bound remote image references need a verified import route before sharing"
                    .into(),
            );
        }
        for key in asset_keys(value) {
            if let Some(path) = value[*key].as_str() {
                if matches!(
                    value["type"].as_str(),
                    Some("localImage" | "local_image" | "localAudio" | "local_audio")
                ) || (!path.starts_with("https:")
                    && !path.starts_with("http:")
                    && !path.starts_with("data:"))
                {
                    paths.insert(path.to_owned());
                }
            }
        }
        match value {
            Value::Array(items) => {
                for item in items {
                    collect(item, paths)?;
                }
            }
            Value::Object(fields) => {
                for item in fields.values() {
                    collect(item, paths)?;
                }
            }
            _ => {}
        }
        Ok(())
    }
    let mut paths = std::collections::HashSet::new();
    for line in fs::read_to_string(path)
        .map_err(|_| "Cannot inventory chat attachments")?
        .lines()
    {
        if line.trim().is_empty() {
            continue;
        }
        let value: Value =
            serde_json::from_str(line).map_err(|_| "Cannot inventory chat attachment records")?;
        collect(&value, &mut paths)?;
    }
    Ok(paths)
}

fn rewrite_assets(
    value: &mut Value,
    replacements: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let keys = asset_keys(value);
    for key in keys {
        if let Some(original) = value[*key].as_str() {
            if let Some(replacement) = replacements.get(original) {
                let replacement = if *key == "path" {
                    replacement.clone()
                } else {
                    url::Url::from_file_path(replacement)
                        .map_err(|_| "Restored image path cannot be represented as a file URL")?
                        .to_string()
                };
                value[*key] = Value::String(replacement);
            }
        }
    }
    match value {
        Value::Array(items) => {
            for item in items {
                rewrite_assets(item, replacements)?;
            }
        }
        Value::Object(fields) => {
            for item in fields.values_mut() {
                rewrite_assets(item, replacements)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_rollout(path: &Path, native_id: &str) -> Result<()> {
    use std::io::{BufRead, BufReader};
    let mut saw_metadata = false;
    let mut count = 0;
    for line in
        BufReader::new(File::open(path).map_err(|_| "Cannot validate native history")?).lines()
    {
        let line = line.map_err(|_| "Cannot validate native history")?;
        if line.trim().is_empty() {
            continue;
        }
        let row: Value = serde_json::from_str(&line)
            .map_err(|_| "Native history contains an incomplete or unsupported record")?;
        if !row.is_object() || row["type"].as_str().is_none() {
            return Err("Native history record format is unsupported".into());
        }
        if !matches!(
            row["type"].as_str(),
            Some(
                "session_meta"
                    | "response_item"
                    | "compacted"
                    | "turn_context"
                    | "event_msg"
                    | "world_state"
                    | "token_usage_record"
                    | "inter_agent_communication_metadata"
                    | "retained_context"
            )
        ) {
            return Err("This native history contains an unverified record type; its original copy is retained".into());
        }
        if count == 0 {
            if row["type"] != "session_meta" || row["payload"]["id"].as_str() != Some(native_id) {
                return Err("Native history identity does not match its checkpoint".into());
            }
            saw_metadata = true;
        }
        count += 1;
    }
    if !saw_metadata {
        return Err("Native history is empty".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (
        tempfile::TempDir,
        ChatLibrary,
        String,
        String,
        PathBuf,
        Value,
    ) {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("app");
        let source = app.join("source");
        fs::create_dir_all(&source).unwrap();
        let profile = Uuid::new_v4().to_string();
        let thread = Uuid::new_v4().to_string();
        let rollout = source.join("rollout.jsonl");
        fs::write(&rollout, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{thread}\"}}}}\n{{\"type\":\"event_msg\",\"payload\":{{\"message\":\"full original context\"}}}}\n")).unwrap();
        let metadata = serde_json::json!({"id":thread,"preview":"fixture","updatedAt":100,"path":rollout,"turns":[{"complete":"history"}]});
        let library = ChatLibrary::open(&app).unwrap();
        (temp, library, profile, thread, rollout, metadata)
    }
    #[test]
    fn failed_catalog_commit_keeps_prior_history_and_retries_after_reopen() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let first = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        let cursor = library.changes(None).unwrap()["cursor"].as_i64().unwrap();
        let mut changed = fs::read(&rollout).unwrap();
        changed.extend_from_slice(
            b"{\"type\":\"event_msg\",\"payload\":{\"message\":\"retained uncommitted work\"}}\n",
        );
        fs::write(&rollout, &changed).unwrap();
        // Fail the second catalog mutation, after the complete checkpoint has
        // been published and the transaction has already updated the thread.
        library
            .connection()
            .unwrap()
            .execute_batch(
                "CREATE TRIGGER fixture_commit_failure BEFORE INSERT ON source_imports
             BEGIN SELECT RAISE(ABORT, 'inert fixture commit failure'); END;",
            )
            .unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .is_err());
        let reopened = ChatLibrary::open(&library.data_root).unwrap();
        let retained = reopened.get(&first.id).unwrap();
        assert_eq!(retained.checkpoint, first.checkpoint);
        assert_eq!(retained.revision, first.revision);
        reopened.checkpoint(&first.checkpoint).unwrap();
        assert!(reopened.changes(Some(cursor)).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(fs::read(&rollout).unwrap(), changed);
        assert_eq!(
            fs::read_dir(reopened.root.join("staging")).unwrap().count(),
            0
        );
        reopened
            .connection()
            .unwrap()
            .execute_batch("DROP TRIGGER fixture_commit_failure")
            .unwrap();
        let recovered = reopened
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        assert_eq!(recovered.id, first.id);
        assert_eq!(recovered.revision, first.revision + 1);
        assert_eq!(reopened.list(false, 0, 100).unwrap().len(), 1);
        let retried = reopened
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        assert_eq!(retried.revision, recovered.revision);
    }

    #[test]
    fn late_attachment_failure_never_commits_partial_history_and_retry_preserves_assets() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        fs::create_dir(root.join("attachments")).unwrap();
        let first_asset = root.join("attachments/first.png");
        let missing_asset = root.join("attachments/second.png");
        fs::write(&first_asset, b"inert complete attachment").unwrap();
        let assets = vec![
            (
                first_asset.to_string_lossy().into_owned(),
                first_asset.clone(),
            ),
            (
                missing_asset.to_string_lossy().into_owned(),
                missing_asset.clone(),
            ),
        ];
        let original = fs::read(&rollout).unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &assets)
            .is_err());
        let reopened = ChatLibrary::open(&library.data_root).unwrap();
        assert!(reopened.list(false, 0, 100).unwrap().is_empty());
        assert!(reopened.changes(None).unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        assert_eq!(fs::read(&rollout).unwrap(), original);
        assert_eq!(
            fs::read_dir(reopened.root.join("staging")).unwrap().count(),
            0
        );
        // The first immutable asset is an unreferenced retained object until
        // retry or explicit collection; no incomplete chat becomes visible.
        assert_eq!(
            fs::read_dir(reopened.root.join("assets")).unwrap().count(),
            1
        );
        fs::write(&missing_asset, b"inert recovered attachment").unwrap();
        let saved = reopened
            .import(&profile, &thread, root, &rollout, &metadata, false, &assets)
            .unwrap();
        assert_eq!(
            reopened.checkpoint(&saved.checkpoint).unwrap().assets.len(),
            2
        );
        assert_eq!(reopened.collect_unreachable().unwrap(), 0);
        let home = library.data_root.join("recovered-home");
        fs::create_dir(&home).unwrap();
        reopened.hydrate(&saved.id, &home).unwrap();
        for (_, source) in assets {
            let hash = files::digest(&source).unwrap();
            assert_eq!(
                fs::read(home.join("shared-assets").join(hash)).unwrap(),
                fs::read(source).unwrap()
            );
        }
    }

    #[test]
    fn library_paging_survives_restart_and_removal_of_the_original_source() {
        let (_temp, library, profile, _thread, rollout, _metadata) = fixture();
        let source = rollout.parent().unwrap();
        let mut active = Vec::new();
        let mut archived = Vec::new();
        for index in 0..267 {
            let thread = Uuid::new_v4().to_string();
            let path = source.join(format!("history-{index}.jsonl"));
            fs::write(&path, format!("{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"{thread}\"}}}}\n{{\"type\":\"event_msg\",\"payload\":{{\"message\":\"retained full turn {index}\"}}}}\n")).unwrap();
            let is_archived = index % 4 == 0;
            let metadata = serde_json::json!({"id":thread,"updatedAt":100,"cwd":source});
            let saved = library
                .import(
                    &profile,
                    &thread,
                    source,
                    &path,
                    &metadata,
                    is_archived,
                    &[],
                )
                .unwrap();
            if is_archived {
                archived.push(saved.id);
            } else {
                active.push(saved.id);
            }
        }
        active.sort();
        archived.sort();
        assert_eq!(library.list(false, 0, u32::MAX).unwrap().len(), 100);
        assert_eq!(library.list(false, 0, 0).unwrap().len(), 1);
        let reopened = ChatLibrary::open(&library.data_root).unwrap();
        for (is_archived, expected) in [(false, &active), (true, &archived)] {
            let mut actual = Vec::new();
            loop {
                let rows = reopened.list(is_archived, actual.len() as u32, 13).unwrap();
                if rows.is_empty() {
                    break;
                }
                actual.extend(rows.into_iter().map(|row| row.id));
            }
            assert_eq!(
                &actual, expected,
                "stable ties must not duplicate or omit pages"
            );
        }
        // Only this inert fixture source is deleted, never an application profile.
        fs::remove_dir_all(source).unwrap();
        let home = library.data_root.join("private account 測試 with spaces");
        fs::create_dir(&home).unwrap();
        let hydrated = reopened.hydrate(&active[0], &home).unwrap();
        assert!(fs::read_to_string(hydrated)
            .unwrap()
            .contains("retained full turn"));
        assert_eq!(reopened.list(false, 0, 100).unwrap().len(), 100);
    }
    #[test]
    fn complete_tool_compaction_and_unicode_history_is_preserved_without_a_summary_substitute() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let rows = [
            serde_json::json!({"type":"session_meta","payload":{"id":thread}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call","call_id":"call-inert","name":"fixture_tool","arguments":"{\"text\":\"測試 complete context\"}"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call-inert","output":"original tool output\nsecond line"}}),
            serde_json::json!({"type":"compacted","payload":{"message":"retained native compaction","replacement_history":[{"role":"user","content":[{"type":"input_text","text":"original user context"}]}]}}),
            serde_json::json!({"type":"inter_agent_communication_metadata","payload":{"source":"fixture-agent","target":"fixture-parent","message":"complete retained coordination"}}),
            serde_json::json!({"type":"retained_context","payload":{"context":"complete retained context 測試","items":[{"role":"user","content":"original context"}]}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"agent_message","message":"after compaction continuation"}}),
        ];
        let original = rows
            .iter()
            .map(|row| serde_json::to_string(row).unwrap() + "\n")
            .collect::<String>();
        fs::write(&rollout, &original).unwrap();
        let saved = library
            .import(
                &profile,
                &thread,
                rollout.parent().unwrap(),
                &rollout,
                &metadata,
                false,
                &[],
            )
            .unwrap();
        let home = library.data_root.join("private-full-history");
        fs::create_dir(&home).unwrap();
        let hydrated = library.hydrate(&saved.id, &home).unwrap();
        assert_eq!(fs::read_to_string(&hydrated).unwrap(), original);
        assert_eq!(fs::read_to_string(&rollout).unwrap(), original);
        let actual: Vec<Value> = fs::read_to_string(hydrated)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(actual, rows);
    }
    #[test]
    fn unknown_native_records_still_block_import_without_changing_the_original() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let original = format!(
            "{}\n{}\n",
            serde_json::json!({"type":"session_meta","payload":{"id":thread}}),
            serde_json::json!({"type":"unknown_future_record","payload":{"context":"keep this intact"}})
        );
        fs::write(&rollout, &original).unwrap();
        assert!(library
            .import(
                &profile,
                &thread,
                rollout.parent().unwrap(),
                &rollout,
                &metadata,
                false,
                &[]
            )
            .unwrap_err()
            .contains("unverified record type"));
        assert_eq!(fs::read_to_string(&rollout).unwrap(), original);
        assert!(library.list(false, 0, 100).unwrap().is_empty());
    }
    #[test]
    fn catalog_events_follow_committed_updates_and_do_not_expose_turns_or_paths() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let baseline = library.changes(None).unwrap();
        let cursor = baseline["cursor"].as_i64().unwrap();
        assert!(baseline["data"].as_array().unwrap().is_empty());
        let imported = library
            .import(
                &profile,
                &thread,
                rollout.parent().unwrap(),
                &rollout,
                &metadata,
                false,
                &[],
            )
            .unwrap();
        let events = library.changes(Some(cursor)).unwrap();
        assert_eq!(events["data"].as_array().unwrap().len(), 1);
        assert_eq!(events["data"][0]["threadId"], imported.id);
        assert_eq!(events["data"][0]["kind"], "updated");
        assert!(events["data"][0]["metadata"].get("turns").is_none());
        assert!(events["data"][0]["metadata"].get("path").is_none());
        assert!(library
            .changes(Some(events["cursor"].as_i64().unwrap()))
            .unwrap()["data"]
            .as_array()
            .unwrap()
            .is_empty());
        assert!(library.changes(Some(-1)).is_err());
    }
    #[test]
    fn full_rollout_import_is_idempotent_and_new_revisions_keep_old_history() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let first = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        let second = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        assert_eq!(first.id, second.id);
        assert_eq!(second.revision, 1);
        assert_eq!(
            fs::read_dir(library.root.join("staging")).unwrap().count(),
            0,
            "Repeated checkpoints must not retain duplicate staging trees"
        );
        let original = fs::read(&rollout).unwrap();
        assert_eq!(
            fs::read(
                library
                    .checkpoint_directory(&first.checkpoint)
                    .unwrap()
                    .join("rollout.jsonl")
            )
            .unwrap(),
            original
        );
        let mut changed = original;
        changed.extend_from_slice(
            b"{\"type\":\"event_msg\",\"payload\":{\"message\":\"next full turn\"}}\n",
        );
        fs::write(&rollout, &changed).unwrap();
        let third = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        assert_eq!(third.revision, 2);
        library.checkpoint(&first.checkpoint).unwrap();
        assert_eq!(library.list(false, 0, 100).unwrap().len(), 1);
    }
    #[test]
    fn legacy_rescan_cannot_overwrite_newer_shared_continuation() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let first = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        let original = fs::read(&rollout).unwrap();
        fs::write(&rollout, [original.clone(), b"{\"type\":\"event_msg\",\"payload\":{\"message\":\"shared account B continuation\"}}\n".to_vec()].concat()).unwrap();
        let newer = library
            .store_checkpoint(&thread, root, &rollout, &[])
            .unwrap();
        library
            .connection()
            .unwrap()
            .execute(
                "UPDATE threads SET checkpoint=?1,revision=2 WHERE id=?2",
                params![newer, first.id],
            )
            .unwrap();
        fs::write(&rollout, &original).unwrap();
        let rescanned = library
            .import(&profile, &thread, root, &rollout, &metadata, true, &[])
            .unwrap();
        assert_eq!(rescanned.checkpoint, newer);
        assert_eq!(rescanned.revision, 2);
        assert!(!rescanned.archived);
        fs::write(
            &rollout,
            [
                original,
                b"{\"type\":\"event_msg\",\"payload\":{\"message\":\"divergent legacy work\"}}\n"
                    .to_vec(),
            ]
            .concat(),
        )
        .unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .is_err());
        assert_eq!(library.get(&thread).unwrap().checkpoint, newer);
    }
    #[test]
    fn source_import_cannot_publish_while_a_writer_holds_the_thread() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let first = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        let writer = crate::file_security::lock_file(
            &library.root.join("leases").join(format!("{thread}.lock")),
        )
        .unwrap();
        // Parallel native fixtures can briefly inherit a prior flock between
        // fork and exec. Wait only for kernel WouldBlock during fixture setup;
        // the product's refusal while the writer is held is still asserted below.
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        loop {
            match fs2::FileExt::try_lock_exclusive(&writer) {
                Ok(()) => break,
                Err(error)
                    if error.kind() == std::io::ErrorKind::WouldBlock
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("Could not acquire inert fixture writer: {error}"),
            }
        }
        let original = fs::read(&rollout).unwrap();
        fs::write(
            &rollout,
            [
                original,
                b"{\"type\":\"event_msg\",\"payload\":{}}\n".to_vec(),
            ]
            .concat(),
        )
        .unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .is_err());
        assert_eq!(library.get(&thread).unwrap().checkpoint, first.checkpoint);
        fs2::FileExt::unlock(&writer).unwrap();
        drop(writer);
        assert_eq!(
            library
                .import(&profile, &thread, root, &rollout, &metadata, false, &[])
                .unwrap()
                .revision,
            2
        );
    }
    #[test]
    fn collisions_keep_both_histories_and_archived_listing_is_separate() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let first = library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .unwrap();
        let second = library
            .import(
                &Uuid::new_v4().to_string(),
                &thread,
                root,
                &rollout,
                &metadata,
                true,
                &[],
            )
            .unwrap();
        assert_ne!(first.id, second.id);
        assert_eq!(library.list(false, 0, 100).unwrap().len(), 1);
        assert_eq!(library.list(true, 0, 100).unwrap().len(), 1);
    }
    #[test]
    fn attachments_are_deduplicated_and_corruption_blocks_hydration() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        let image = root.join("assets/image.png");
        fs::write(&image, b"fixture image bytes").unwrap();
        let saved = library
            .import(
                &profile,
                &thread,
                root,
                &rollout,
                &metadata,
                false,
                &[("original-image".into(), image)],
            )
            .unwrap();
        let checkpoint = library.checkpoint(&saved.checkpoint).unwrap();
        assert_eq!(checkpoint.assets.len(), 1);
        fs::write(
            library
                .root
                .join("assets")
                .join(&checkpoint.assets[0].digest),
            b"corrupt",
        )
        .unwrap();
        assert!(library.checkpoint(&saved.checkpoint).is_err());
        let home = library.data_root.join("private-home");
        fs::create_dir(&home).unwrap();
        assert!(library.hydrate(&saved.id, &home).is_err());
        assert!(!home.join("sessions").exists());
    }
    #[test]
    fn required_local_image_is_restored_and_unmapped_assets_block_import() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        let image = root.join("assets/image.png");
        fs::write(&image, b"complete image").unwrap();
        let reference = image.to_str().unwrap().to_owned();
        use std::io::Write;
        let row = serde_json::json!({"type":"response_item","payload":{"type":"localImage","path":reference}});
        writeln!(
            fs::OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{row}"
        )
        .unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .is_err());
        assert!(library.list(false, 0, 100).unwrap().is_empty());
        let saved = library
            .import(
                &profile,
                &thread,
                root,
                &rollout,
                &metadata,
                false,
                &[(reference.clone(), image)],
            )
            .unwrap();
        let home = library.data_root.join("private-home");
        fs::create_dir(&home).unwrap();
        let hydrated = library.hydrate(&saved.id, &home).unwrap();
        let paths = local_assets(&hydrated).unwrap();
        assert_eq!(paths.len(), 1);
        let path = Path::new(paths.iter().next().unwrap());
        assert!(path.starts_with(home.join("shared-assets")));
        assert_eq!(fs::read(path).unwrap(), b"complete image");
        let canonical = fs::read_to_string(
            library
                .checkpoint_directory(&saved.checkpoint)
                .unwrap()
                .join("rollout.jsonl"),
        )
        .unwrap();
        assert!(canonical.lines().any(|line| {
            let row: Value = serde_json::from_str(line).unwrap();
            row["payload"]["path"].as_str() == Some(reference.as_str())
        }));
    }

    #[test]
    fn credential_files_cannot_be_imported_as_shared_attachments() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        let auth = root.join("auth.json");
        fs::write(&auth, b"private fixture credential").unwrap();
        assert!(library
            .import(
                &profile,
                &thread,
                root,
                &rollout,
                &metadata,
                false,
                &[("credential-reference".into(), auth)]
            )
            .is_err());
        assert_eq!(
            fs::read_dir(library.root.join("assets")).unwrap().count(),
            0
        );
        assert!(library.list(false, 0, 100).unwrap().is_empty());
    }

    #[test]
    fn file_url_attachments_keep_valid_urls_when_hydrated() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        fs::create_dir_all(root.join("assets")).unwrap();
        let image = root.join("assets/image with space.png");
        fs::write(&image, b"image bytes").unwrap();
        let reference = url::Url::from_file_path(&image).unwrap().to_string();
        use std::io::Write;
        let row = serde_json::json!({"type":"response_item","payload":{"type":"input_image","image_url":reference}});
        writeln!(
            fs::OpenOptions::new().append(true).open(&rollout).unwrap(),
            "{row}"
        )
        .unwrap();
        let saved = library
            .import(
                &profile,
                &thread,
                root,
                &rollout,
                &metadata,
                false,
                &[(reference, image)],
            )
            .unwrap();
        let home = library.data_root.join("private home");
        fs::create_dir(&home).unwrap();
        let hydrated = library.hydrate(&saved.id, &home).unwrap();
        let paths = local_assets(&hydrated).unwrap();
        let path = paths.iter().next().unwrap();
        assert!(path.starts_with("file:"));
        assert!(path.contains("%20"));
        let local = url::Url::parse(path).unwrap().to_file_path().unwrap();
        assert_eq!(fs::read(local).unwrap(), b"image bytes");
    }

    #[test]
    fn invalid_history_and_database_links_never_become_import_success() {
        let (_temp, library, profile, thread, rollout, metadata) = fixture();
        let root = rollout.parent().unwrap();
        fs::write(&rollout, b"{incomplete\n").unwrap();
        assert!(library
            .import(&profile, &thread, root, &rollout, &metadata, false, &[])
            .is_err());
        assert!(library.list(false, 0, 100).unwrap().is_empty());
        #[cfg(unix)]
        {
            let journal = library.root.join("catalog.sqlite-journal");
            std::os::unix::fs::symlink(&rollout, &journal).unwrap();
            assert!(library.connection().is_err());
        }
    }
}
