//! Resumable pre-migration snapshots. This service never removes original data.
use crate::managed_files as files;
use crate::profiles::{self, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

const SCHEMA: u32 = 1;
const SPACE_RESERVE: u64 = 16 * 1024 * 1024;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum SnapshotState {
    Copying,
    Verified,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SnapshotRecord {
    pub schema: u32,
    pub profile_id: String,
    pub snapshot_id: String,
    pub state: SnapshotState,
    pub source_digest: String,
    pub metadata_digest: String,
    #[serde(default)]
    pub dependency_digest: Option<String>,
    pub source_bytes: u64,
    pub entries: u64,
    pub created_at: DateTime<Utc>,
    pub verified_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationPreview {
    pub profile_id: String,
    pub source_bytes: u64,
    pub entries: u64,
    pub required_backup_bytes: u64,
    pub available_backup_bytes: u64,
    pub required_app_bytes: u64,
    pub available_app_bytes: u64,
    pub external_links: Vec<PathBuf>,
    pub running: bool,
    pub snapshot: Option<SnapshotRecord>,
}

pub struct MigrationStore {
    data_root: PathBuf,
    backups: PathBuf,
    journals: PathBuf,
}

fn valid_id(id: &str) -> Result<()> {
    if Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id) {
        Ok(())
    } else {
        Err("Invalid migration identifier".into())
    }
}

impl MigrationStore {
    pub fn open(data_root: &Path) -> Result<Self> {
        files::root(data_root)?;
        let parent = data_root.parent().ok_or("App data has no backup parent")?;
        files::root(parent)?;
        let backups = parent.join("multi-codex-backups");
        files::directory(parent, &backups)?;
        // Backup data must be independent of account deletion and app-owned GC.
        if backups.starts_with(data_root) || data_root.starts_with(&backups) {
            return Err("Backup and application storage must be disjoint".into());
        }
        let journals = data_root.join("migrations/profiles");
        files::directory(data_root, &journals)?;
        Ok(Self {
            data_root: data_root.into(),
            backups,
            journals,
        })
    }

    fn source(&self, id: &str) -> Result<PathBuf> {
        valid_id(id)?;
        let path = files::checked(
            &self.data_root,
            &self.data_root.join("profiles").join(id),
            false,
        )?;
        files::root(&path)?;
        Ok(path)
    }

    fn record_path(&self, id: &str) -> Result<PathBuf> {
        valid_id(id)?;
        files::checked(
            &self.data_root,
            &self.journals.join(format!("{id}.json")),
            true,
        )
    }

    fn save(&self, record: &SnapshotRecord) -> Result<()> {
        profiles::write_private_file(
            &self.record_path(&record.profile_id)?,
            &serde_json::to_vec_pretty(record).map_err(|_| "Could not encode migration journal")?,
        )
    }

    pub fn record(&self, id: &str) -> Result<Option<SnapshotRecord>> {
        let path = self.record_path(id)?;
        if !path.exists() {
            return Ok(None);
        }
        let bytes = fs::read(files::regular(&self.data_root, &path)?)
            .map_err(|_| "Could not read migration journal")?;
        let record: SnapshotRecord = serde_json::from_slice(&bytes)
            .map_err(|_| "Migration journal is invalid; original data has been retained")?;
        if record.schema != SCHEMA || record.profile_id != id {
            return Err("Migration journal belongs to another profile or schema".into());
        }
        valid_id(&record.snapshot_id)?;
        Ok(Some(record))
    }

    fn snapshot_directory(&self, record: &SnapshotRecord) -> Result<PathBuf> {
        valid_id(&record.profile_id)?;
        valid_id(&record.snapshot_id)?;
        files::checked(
            &self.backups,
            &self.backups.join(format!(
                "profile-{}-{}",
                record.profile_id, record.snapshot_id
            )),
            false,
        )
    }

    pub fn snapshot_data(&self, record: &SnapshotRecord) -> Result<PathBuf> {
        let directory = self.snapshot_directory(record)?;
        files::checked(&self.backups, &directory.join("data"), false)
    }

    pub fn import_home(&self, record: &SnapshotRecord) -> Result<tempfile::TempDir> {
        self.verify(record)?;
        let root = self.data_root.join("migrations/import-homes");
        files::directory(&self.data_root, &root)?;
        let temporary =
            tempfile::tempdir_in(root).map_err(|_| "Cannot allocate stopped history copy")?;
        profiles::set_owner_only_dir(temporary.path())?;
        let destination = temporary.path().join("codex-home");
        let source = self.snapshot_data(record)?.join("codex-home");
        files::root(&source)?;
        files::copy_tree(&source, &destination)?;
        if let Some(digest) = &record.dependency_digest {
            let snapshot = self.snapshot_directory(record)?;
            let dependencies = crate::migration_dependencies::verify(&snapshot, digest)?;
            crate::migration_dependencies::rebase_subtree(
                &snapshot,
                &destination,
                &dependencies,
                Path::new("codex-home"),
            )?;
        }
        Ok(temporary)
    }

    pub fn verify(&self, record: &SnapshotRecord) -> Result<()> {
        if record.schema != SCHEMA
            || record.state != SnapshotState::Verified
            || record.verified_at.is_none()
        {
            return Err("Snapshot is incomplete and cannot authorize migration".into());
        }
        let directory = self.snapshot_directory(record)?;
        let marker = files::regular(&self.backups, &directory.join("data.verified.json"))?;
        let rows: Vec<(PathBuf, String)> = serde_json::from_slice(
            &fs::read(marker).map_err(|_| "Could not read snapshot manifest")?,
        )
        .map_err(|_| "Snapshot manifest is invalid")?;
        if files::manifest_digest(&rows)? != record.source_digest
            || rows != files::tree_manifest(&self.snapshot_data(record)?)?
            || files::digest(&files::regular(
                &self.backups,
                &directory.join("profile.json"),
            )?)? != record.metadata_digest
        {
            return Err("Snapshot content no longer matches its verified manifest".into());
        }
        if let Some(digest) = &record.dependency_digest {
            crate::migration_dependencies::verify(&directory, digest)?;
        }
        Ok(())
    }

    pub fn preview(&self, id: &str, running: bool) -> Result<MigrationPreview> {
        let source = self.source(id)?;
        let (profile_bytes, entries, _) = inventory(&source)?;
        let (_, external_links, dependency_bytes) =
            crate::migration_dependencies::inventory(&self.data_root, &source)?;
        let source_bytes = profile_bytes
            .checked_add(dependency_bytes)
            .ok_or("Migration inventory is too large")?;
        let required_backup_bytes = backup_requirement(source_bytes, entries)?;
        // Conservatively reserve package installation/publication, private catalog
        // copies, chat/asset checkpoints, and the backup if it shares this volume.
        let required_app_bytes = source_bytes
            .checked_mul(2)
            .and_then(|value| value.checked_add(4 * 1024 * 1024 * 1024))
            .and_then(|value| value.checked_add(required_backup_bytes))
            .ok_or("Migration inventory is too large")?;
        Ok(MigrationPreview {
            profile_id: id.into(),
            source_bytes,
            entries,
            required_backup_bytes,
            available_backup_bytes: fs2::available_space(&self.backups)
                .map_err(|_| "Could not check backup disk space")?,
            required_app_bytes,
            available_app_bytes: fs2::available_space(&self.data_root)
                .map_err(|_| "Could not check migration disk space")?,
            external_links,
            running,
            snapshot: self.record(id)?,
        })
    }

    pub fn preview_restore(&self, id: &str, running: bool) -> Result<MigrationPreview> {
        let snapshot = self
            .record(id)?
            .ok_or("No verified restore snapshot is available")?;
        self.verify(&snapshot)?;
        Ok(MigrationPreview {
            profile_id: id.into(),
            source_bytes: snapshot.source_bytes,
            entries: snapshot.entries,
            required_backup_bytes: 0,
            available_backup_bytes: fs2::available_space(&self.backups)
                .map_err(|_| "Cannot check backup space")?,
            required_app_bytes: 0,
            available_app_bytes: fs2::available_space(&self.data_root)
                .map_err(|_| "Cannot check restore space")?,
            external_links: vec![],
            running,
            snapshot: Some(snapshot),
        })
    }

    /// Caller holds profile launch/credential/session guards throughout this operation.
    /// Recheck closed state around copying, since externally launched clients bypass app locks.
    pub fn prepare(
        &self,
        id: &str,
        profile_metadata: &[u8],
        check_closed: impl Fn() -> Result<()>,
    ) -> Result<SnapshotRecord> {
        check_closed()?;
        let source = self.source(id)?;
        let operation_path = files::checked(
            &self.data_root,
            &self.data_root.join("migrations/operation.lock"),
            true,
        )?;
        let _operation = exclusive(&operation_path)?;
        check_closed()?;
        let preview = self.preview(id, false)?;
        if !preview.external_links.is_empty() {
            return Err("This profile has linked files outside verified snapshot coverage. Keep its original layout until those dependencies are resolved".into());
        }
        if preview.available_backup_bytes < preview.required_backup_bytes {
            return Err(
                "Not enough free space for a verified backup. Original profile data has been kept"
                    .into(),
            );
        }
        let (dependencies, _, _) =
            crate::migration_dependencies::inventory(&self.data_root, &source)?;
        let dependency_digest = {
            use sha2::{Digest, Sha256};
            format!(
                "{:x}",
                Sha256::digest(crate::migration_dependencies::encode(&dependencies)?)
            )
        };
        let rows = files::tree_manifest(&source)?;
        let source_digest = files::manifest_digest(&rows)?;
        let metadata_digest = {
            use sha2::{Digest, Sha256};
            format!("{:x}", Sha256::digest(profile_metadata))
        };
        if let Some(previous) = self.record(id)? {
            if previous.state == SnapshotState::Verified
                && previous.source_digest == source_digest
                && previous.metadata_digest == metadata_digest
                && previous.dependency_digest.as_deref() == Some(&dependency_digest)
            {
                self.verify(&previous)?;
                check_closed()?;
                return Ok(previous);
            }
            // An incomplete/older snapshot is never overwritten or deleted on retry.
        }
        let mut record = SnapshotRecord {
            schema: SCHEMA,
            profile_id: id.into(),
            snapshot_id: Uuid::new_v4().to_string(),
            state: SnapshotState::Copying,
            source_digest,
            metadata_digest,
            dependency_digest: Some(dependency_digest),
            source_bytes: preview.source_bytes,
            entries: preview.entries,
            created_at: Utc::now(),
            verified_at: None,
        };
        let destination = self
            .backups
            .join(format!("profile-{id}-{}", record.snapshot_id));
        files::directory(&self.backups, &destination)?;
        self.save(&record)?;
        profiles::write_private_file(&destination.join("profile.json"), profile_metadata)?;
        files::verified_snapshot(&source, &destination.join("data"))?;
        crate::migration_dependencies::freeze(&self.data_root, &destination, &dependencies)?;
        check_closed()?;
        if record.source_digest != files::manifest_digest(&files::tree_manifest(&source)?)?
            || dependencies != crate::migration_dependencies::inventory(&self.data_root, &source)?.0
        {
            return Err("Profile changed during backup. Migration has not started; close sessions and retry".into());
        }
        record.state = SnapshotState::Verified;
        record.verified_at = Some(Utc::now());
        self.verify(&record)?;
        self.save(&record)?;
        Ok(record)
    }

    /// Restore to a new staging directory only. Never overwrite a current profile.
    pub fn stage_restore(
        &self,
        id: &str,
        destination: &Path,
        check_closed: impl Fn() -> Result<()>,
    ) -> Result<()> {
        check_closed()?;
        let operation_path = files::checked(
            &self.data_root,
            &self.data_root.join("migrations/operation.lock"),
            true,
        )?;
        let _operation = exclusive(&operation_path)?;
        let record = self
            .record(id)?
            .ok_or("No migration snapshot is available")?;
        self.verify(&record)?;
        let restore_root = self.data_root.join("migrations/restores");
        files::directory(&self.data_root, &restore_root)?;
        let destination = files::checked(&restore_root, destination, true)?;
        if destination.parent() != Some(restore_root.as_path()) {
            return Err("Restore destination must be a new migration staging directory".into());
        }
        files::verified_snapshot(&self.snapshot_data(&record)?, &destination)?;
        if let Some(digest) = &record.dependency_digest {
            let snapshot = self.snapshot_directory(&record)?;
            let dependencies = crate::migration_dependencies::verify(&snapshot, digest)?;
            crate::migration_dependencies::rebase(&snapshot, &destination, &dependencies)?;
            profiles::write_private_file(
                &destination.with_extension("verified.json"),
                &serde_json::to_vec(&files::tree_manifest(&destination)?)
                    .map_err(|_| "Cannot encode restored manifest")?,
            )?;
        }
        check_closed()?;
        Ok(())
    }
}

fn exclusive(path: &Path) -> Result<crate::file_security::ExclusiveLock> {
    let file = crate::file_security::lock_file(path)?;
    crate::file_security::exclusive_lock(
        file,
        "Another storage migration is in progress; retry when it finishes",
    )
}

fn backup_requirement(bytes: u64, entries: u64) -> Result<u64> {
    bytes
        .checked_add(
            entries
                .checked_mul(4096)
                .ok_or("Backup inventory is too large")?,
        )
        .and_then(|bytes| bytes.checked_add(SPACE_RESERVE))
        .ok_or("Backup inventory is too large".into())
}

fn normalized(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => {
                normalized.pop();
            }
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn inventory(source: &Path) -> Result<(u64, u64, Vec<PathBuf>)> {
    fn walk(
        source: &Path,
        path: &Path,
        bytes: &mut u64,
        entries: &mut u64,
        external: &mut Vec<PathBuf>,
    ) -> Result<()> {
        *entries = entries
            .checked_add(1)
            .ok_or("Profile inventory is too large")?;
        let metadata =
            fs::symlink_metadata(path).map_err(|_| "Could not inspect profile inventory")?;
        if files::is_link(&metadata) {
            let target = files::link_target(path)?;
            let resolved = normalized(&path.parent().ok_or("Invalid link parent")?.join(&target));
            // Absolute internal links still point at the original tree after copying.
            // Preserve them in the source and require an explicit rebase policy before migration.
            if target.is_absolute()
                || !resolved.starts_with(source)
                || fs::canonicalize(&resolved).is_err()
                || !fs::canonicalize(&resolved)
                    .map_err(|_| "Cannot resolve profile link")?
                    .starts_with(
                        fs::canonicalize(source).map_err(|_| "Cannot resolve profile root")?,
                    )
            {
                external.push(
                    path.strip_prefix(source)
                        .map_err(|_| "Invalid linked profile path")?
                        .into(),
                );
            }
        } else if metadata.is_dir() {
            for entry in fs::read_dir(path).map_err(|_| "Could not inspect profile inventory")? {
                walk(
                    source,
                    &entry
                        .map_err(|_| "Could not inspect profile inventory")?
                        .path(),
                    bytes,
                    entries,
                    external,
                )?;
            }
        } else if metadata.is_file() {
            *bytes = bytes
                .checked_add(metadata.len())
                .ok_or("Profile inventory is too large")?;
        } else {
            return Err(
                "Close sessions and resolve transient profile entries before migration".into(),
            );
        }
        Ok(())
    }
    let mut bytes = 0;
    let mut entries = 0;
    let mut external = Vec::new();
    walk(source, source, &mut bytes, &mut entries, &mut external)?;
    external.sort();
    Ok((bytes, entries, external))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, MigrationStore, String, PathBuf) {
        let temp = tempfile::tempdir().unwrap();
        let app = fs::canonicalize(temp.path()).unwrap().join("app");
        let id = Uuid::new_v4().to_string();
        let profile = app.join("profiles").join(&id);
        fs::create_dir_all(profile.join("codex-home/sessions")).unwrap();
        fs::write(
            profile.join("codex-home/sessions/thread.jsonl"),
            "full history",
        )
        .unwrap();
        let store = MigrationStore::open(&app).unwrap();
        (temp, store, id, profile)
    }
    #[test]
    fn stopped_snapshot_is_reusable_and_corruption_blocks_restore() {
        let (_temp, store, id, profile) = fixture();
        let first = store.prepare(&id, b"{}", || Ok(())).unwrap();
        let second = store.prepare(&id, b"{}", || Ok(())).unwrap();
        assert_eq!(first.snapshot_id, second.snapshot_id);
        fs::write(
            store
                .snapshot_data(&first)
                .unwrap()
                .join("codex-home/sessions/thread.jsonl"),
            "broken",
        )
        .unwrap();
        assert!(store.verify(&first).is_err());
        assert!(store
            .stage_restore(
                &id,
                &store.data_root.join("migrations/restores/recovery"),
                || Ok(())
            )
            .is_err());
        assert_eq!(
            fs::read_to_string(profile.join("codex-home/sessions/thread.jsonl")).unwrap(),
            "full history"
        );
    }
    #[cfg(unix)]
    #[test]
    fn verified_snapshot_detects_changed_owner_file_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let (_temp, store, id, profile) = fixture();
        let binary = profile.join("tool");
        fs::write(&binary, b"fixture executable").unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let snapshot = store.prepare(&id, b"{}", || Ok(())).unwrap();
        let copied = store.snapshot_data(&snapshot).unwrap().join("tool");
        assert_eq!(
            fs::metadata(&copied).unwrap().permissions().mode() & 0o700,
            0o700
        );
        fs::set_permissions(&copied, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(store.verify(&snapshot).is_err());
        assert_eq!(
            fs::metadata(&binary).unwrap().permissions().mode() & 0o700,
            0o700
        );
    }

    #[test]
    fn source_changes_get_new_snapshot_and_old_backup_is_retained() {
        let (_temp, store, id, profile) = fixture();
        let first = store.prepare(&id, b"{}", || Ok(())).unwrap();
        fs::write(profile.join("new-chat"), "new work").unwrap();
        let second = store.prepare(&id, b"{}", || Ok(())).unwrap();
        assert_ne!(first.snapshot_id, second.snapshot_id);
        store.verify(&first).unwrap();
        store.verify(&second).unwrap();
    }
    #[test]
    fn client_starting_during_backup_leaves_only_incomplete_journal() {
        let (_temp, store, id, _profile) = fixture();
        let calls = std::cell::Cell::new(0);
        let result = store.prepare(&id, b"{}", || {
            calls.set(calls.get() + 1);
            if calls.get() >= 3 {
                Err("Client started".into())
            } else {
                Ok(())
            }
        });
        assert!(result.is_err());
        let record = store.record(&id).unwrap().unwrap();
        assert_eq!(record.state, SnapshotState::Copying);
        assert!(store.verify(&record).is_err());
        let retry = store.prepare(&id, b"{}", || Ok(())).unwrap();
        assert_ne!(record.snapshot_id, retry.snapshot_id);
        assert!(store.snapshot_data(&record).unwrap().exists());
    }
    #[test]
    fn restore_is_verified_staging_and_never_overwrites_current_work() {
        let (_temp, store, id, profile) = fixture();
        store.prepare(&id, b"{}", || Ok(())).unwrap();
        fs::write(profile.join("new-work"), "keep").unwrap();
        let staged = store.data_root.join("migrations/restores/recovery");
        store.stage_restore(&id, &staged, || Ok(())).unwrap();
        assert!(staged.join("codex-home/sessions/thread.jsonl").exists());
        assert_eq!(
            fs::read_to_string(profile.join("new-work")).unwrap(),
            "keep"
        );
        assert!(store.stage_restore(&id, &profile, || Ok(())).is_err());
        assert!(store.stage_restore(&id, &staged, || Ok(())).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn external_and_absolute_links_block_migration_without_modifying_source() {
        let (_temp, store, id, profile) = fixture();
        std::os::unix::fs::symlink("../../external", profile.join("external-link")).unwrap();
        assert_eq!(
            store.preview(&id, false).unwrap().external_links,
            vec![PathBuf::from("external-link")]
        );
        assert!(store.prepare(&id, b"{}", || Ok(())).is_err());
        assert!(store.record(&id).unwrap().is_none());
        assert_eq!(
            fs::read_link(profile.join("external-link")).unwrap(),
            PathBuf::from("../../external")
        );
    }
    #[test]
    fn rejects_journal_profile_confusion_and_invalid_ids() {
        let (_temp, store, id, _profile) = fixture();
        let mut record = store.prepare(&id, b"{}", || Ok(())).unwrap();
        record.profile_id = Uuid::new_v4().to_string();
        profiles::write_private_file(
            &store.record_path(&id).unwrap(),
            &serde_json::to_vec(&record).unwrap(),
        )
        .unwrap();
        assert!(store.record(&id).is_err());
        assert!(store.preview("../escape", false).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn owned_links_are_frozen_and_restore_is_independent_of_original_targets() {
        let (_temp, store, id, profile) = fixture();
        let payload = store
            .data_root
            .join("shared-vscode/generations/fixture/package");
        fs::create_dir_all(&payload).unwrap();
        fs::write(payload.join("package.json"), "original package").unwrap();
        std::os::unix::fs::symlink(&payload, profile.join("catalog-package")).unwrap();
        std::os::unix::fs::symlink(
            profile.join("codex-home/sessions/thread.jsonl"),
            profile.join("absolute-history"),
        )
        .unwrap();
        fs::create_dir_all(profile.join("codex-home/assets")).unwrap();
        fs::create_dir_all(profile.join("codex-home/attachments")).unwrap();
        let image = profile.join("codex-home/attachments/image.png");
        fs::write(&image, "original image").unwrap();
        std::os::unix::fs::symlink(&image, profile.join("codex-home/assets/linked-image.png"))
            .unwrap();
        assert!(store.preview(&id, false).unwrap().external_links.is_empty());
        let snapshot = store.prepare(&id, b"{}", || Ok(())).unwrap();
        fs::write(payload.join("package.json"), "new generation bytes").unwrap();
        fs::write(
            profile.join("codex-home/sessions/thread.jsonl"),
            "later history",
        )
        .unwrap();
        store.verify(&snapshot).unwrap();
        fs::write(&image, "new image").unwrap();
        let imported = store.import_home(&snapshot).unwrap();
        assert_eq!(
            fs::read_to_string(imported.path().join("codex-home/assets/linked-image.png")).unwrap(),
            "original image"
        );
        let destination = store.data_root.join("migrations/restores/independent");
        store.stage_restore(&id, &destination, || Ok(())).unwrap();
        assert_eq!(
            fs::read_to_string(destination.join("catalog-package/package.json")).unwrap(),
            "original package"
        );
        assert_eq!(
            fs::read_to_string(destination.join("absolute-history")).unwrap(),
            "full history"
        );
        assert!(!fs::symlink_metadata(destination.join("catalog-package"))
            .unwrap()
            .file_type()
            .is_symlink());
        assert!(fs::symlink_metadata(profile.join("catalog-package"))
            .unwrap()
            .file_type()
            .is_symlink());
        let second = store.prepare(&id, b"{}", || Ok(())).unwrap();
        assert_ne!(snapshot.snapshot_id, second.snapshot_id);
        fs::write(
            store
                .snapshot_directory(&snapshot)
                .unwrap()
                .join("dependencies/0"),
            "corrupt",
        )
        .unwrap();
        assert!(store.verify(&snapshot).is_err());
    }
}
