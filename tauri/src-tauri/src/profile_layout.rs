//! Durable layout selection. Retained legacy folders are never renamed or deleted.
use crate::{
    managed_files as files, managed_settings,
    migration::SnapshotRecord,
    profiles::{self, Result},
    shared_vscode::{ExtensionStore, Generation},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LayoutPhase {
    Prepared,
    SettingsApplied,
    Committed,
    RollbackPending,
    RolledBack,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LayoutRecord {
    pub schema: u32,
    pub profile_id: String,
    pub operation_id: String,
    pub phase: LayoutPhase,
    pub snapshot: SnapshotRecord,
    #[serde(default)]
    pub legacy_snapshot: Option<SnapshotRecord>,
    pub generation: Generation,
    pub catalog_name: String,
    pub settings_changes: Vec<managed_settings::SettingChange>,
    #[serde(default)]
    pub launch_confirmed: bool,
}

pub struct LayoutStore {
    data_root: PathBuf,
}
fn id(value: &str) -> Result<()> {
    if Uuid::parse_str(value).is_ok_and(|uuid| uuid.to_string() == value) {
        Ok(())
    } else {
        Err("Invalid layout identifier".into())
    }
}

impl LayoutStore {
    pub fn open(data_root: &Path) -> Result<Self> {
        files::directory(data_root, &data_root.join("migrations/layouts"))?;
        Ok(Self {
            data_root: data_root.into(),
        })
    }
    fn path(&self, profile_id: &str) -> Result<PathBuf> {
        id(profile_id)?;
        files::checked(
            &self.data_root,
            &self
                .data_root
                .join("migrations/layouts")
                .join(format!("{profile_id}.json")),
            true,
        )
    }
    fn save(&self, record: &LayoutRecord) -> Result<()> {
        profiles::write_private_file(
            &self.path(&record.profile_id)?,
            &serde_json::to_vec_pretty(record).map_err(|_| "Could not encode layout journal")?,
        )
    }
    fn lock(&self, profile_id: &str) -> Result<crate::file_security::ExclusiveLock> {
        let path = self.path(profile_id)?.with_extension("lock");
        files::checked(&self.data_root, &path, true)?;
        let file = crate::file_security::lock_file(&path)?;
        crate::file_security::exclusive_lock(
            file,
            "A profile layout operation is already in progress",
        )
    }
    pub fn record(&self, profile_id: &str) -> Result<Option<LayoutRecord>> {
        let path = self.path(profile_id)?;
        if !path.exists() {
            return Ok(None);
        }
        let record: LayoutRecord = serde_json::from_slice(
            &fs::read(files::regular(&self.data_root, &path)?)
                .map_err(|_| "Could not read layout journal")?,
        )
        .map_err(|_| "Layout journal is invalid; repair it before launching this profile")?;
        id(&record.operation_id)?;
        id(&record.catalog_name)?;
        if record.schema != 1
            || record.profile_id != profile_id
            || record.snapshot.profile_id != profile_id
        {
            return Err("Layout journal belongs to another profile or schema".into());
        }
        Ok(Some(record))
    }
    fn settings_path(&self, profile_id: &str) -> Result<PathBuf> {
        id(profile_id)?;
        let directory = self
            .data_root
            .join("profiles")
            .join(profile_id)
            .join("vscode-user-data/User");
        files::directory(&self.data_root, &directory)?;
        files::checked(&self.data_root, &directory.join("settings.json"), true)
    }
    fn settings(&self, path: &Path) -> Result<String> {
        if !path.exists() {
            return Ok("{}\n".into());
        }
        fs::read_to_string(files::regular(&self.data_root, path)?)
            .map_err(|_| "Could not read private editor settings".into())
    }
    fn catalog(&self, record: &LayoutRecord) -> Result<PathBuf> {
        id(&record.catalog_name)?;
        files::checked(
            &self.data_root,
            &self
                .data_root
                .join("profiles")
                .join(&record.profile_id)
                .join("extension-catalogs")
                .join(&record.catalog_name),
            false,
        )
    }
    pub fn selected_catalog(&self, profile_id: &str) -> Result<Option<PathBuf>> {
        crate::profile_restore::ensure_ready(&self.data_root, profile_id)?;
        let Some(record) = self.record(profile_id)? else {
            return Ok(None);
        };
        match record.phase {
            LayoutPhase::Committed => {
                let store = ExtensionStore::open(&self.data_root)?;
                store.verify(&record.generation)?;
                let catalog = self.catalog(&record)?;
                store.verify_catalog(&record.generation, &catalog)?;
                Ok(Some(catalog))
            }
            LayoutPhase::RolledBack => Ok(None),
            _ => Err("A storage change was interrupted. Close this profile and recover its migration before launching".into()),
        }
    }

    pub fn commit(
        &self,
        snapshot: &SnapshotRecord,
        generation: &Generation,
        desired_settings: &BTreeMap<String, Value>,
        closed: impl Fn() -> Result<()>,
    ) -> Result<LayoutRecord> {
        let _lock = self.lock(&snapshot.profile_id)?;
        closed()?;
        let previous = self.record(&snapshot.profile_id)?;
        if let Some(record) = &previous {
            if record.phase == LayoutPhase::Committed {
                if record.generation.id == generation.id
                    && self.selected_catalog(&record.profile_id).is_ok()
                {
                    return Ok(record.clone());
                }
            } else if record.phase != LayoutPhase::RolledBack {
                return Err("Recover the interrupted migration before retrying".into());
            }
        }
        crate::migration::MigrationStore::open(&self.data_root)?.verify(snapshot)?;
        let source = self.data_root.join("profiles").join(&snapshot.profile_id);
        if files::manifest_digest(&files::tree_manifest(&source)?)? != snapshot.source_digest {
            return Err("Profile data changed after backup; prepare a fresh verified backup before migration".into());
        }
        let legacy = self
            .data_root
            .join("profiles")
            .join(&snapshot.profile_id)
            .join("vscode-extensions");
        let store = ExtensionStore::open(&self.data_root)?;
        let settings_path = self.settings_path(&snapshot.profile_id)?;
        let original = self.settings(&settings_path)?;
        let parsed = jsonc_parser::parse_to_serde_value::<Value>(
            &original,
            &jsonc_parser::ParseOptions::default(),
        )
        .map_err(|_| "Malformed editor settings; keep the legacy layout")?;
        let executable = &parsed["chatgpt.cliExecutable"];
        let managed_executable = previous
            .as_ref()
            .filter(|record| record.phase == LayoutPhase::Committed)
            .and_then(|record| {
                record
                    .settings_changes
                    .iter()
                    .find(|change| change.key == "chatgpt.cliExecutable")
            })
            .map(|change| &change.installed);
        if !executable.is_null()
            && executable.as_str() != Some("")
            && managed_executable != Some(executable)
        {
            return Err("This profile has a custom Codex executable. Keep the legacy layout until its owner restores the bundled executable".into());
        }
        let (installed, mut changes) = managed_settings::install(&original, desired_settings)?;
        if let Some(previous) = previous
            .as_ref()
            .filter(|record| record.phase == LayoutPhase::Committed)
        {
            for old in &previous.settings_changes {
                if let Some(new) = changes.iter_mut().find(|change| change.key == old.key) {
                    if new.previous.as_ref() == Some(&old.installed) {
                        new.previous = old.previous.clone();
                    }
                } else {
                    changes.push(old.clone());
                }
            }
        }
        let catalog = store.stage_catalog_preserving(
            generation,
            &snapshot.profile_id,
            if let Some(record) = previous
                .as_ref()
                .filter(|record| record.phase == LayoutPhase::Committed)
            {
                Some(self.catalog(record)?)
            } else {
                legacy.exists().then_some(legacy.clone())
            }
            .as_deref(),
        )?;
        closed()?;
        // Exclude only the catalog this operation just generated. Every retained
        // source entry must still match the verified closed-profile snapshot.
        let migrations = crate::migration::MigrationStore::open(&self.data_root)?;
        let before = files::tree_manifest(&migrations.snapshot_data(snapshot)?)?;
        let added = catalog
            .strip_prefix(&source)
            .map_err(|_| "Catalog escaped its profile")?;
        let had_catalog_root = before
            .iter()
            .any(|(path, _)| path == Path::new("extension-catalogs"));
        let current: Vec<_> = files::tree_manifest(&source)?
            .into_iter()
            .filter(|(path, _)| {
                !path.starts_with(added)
                    && (had_catalog_root || path != Path::new("extension-catalogs"))
            })
            .collect();
        if current != before {
            return Err("Profile files changed while preparing shared storage. The original layout and verified backup are retained; close clients and retry".into());
        }
        if self.settings(&settings_path)? != original {
            return Err(
                "Editor settings changed during migration; the legacy layout remains selected"
                    .into(),
            );
        }
        files::root(&source)?;
        let mut record = LayoutRecord {
            schema: 1,
            profile_id: snapshot.profile_id.clone(),
            operation_id: Uuid::new_v4().to_string(),
            phase: LayoutPhase::Prepared,
            snapshot: snapshot.clone(),
            legacy_snapshot: Some(
                previous
                    .as_ref()
                    .filter(|record| record.phase == LayoutPhase::Committed)
                    .and_then(|record| record.legacy_snapshot.clone())
                    .unwrap_or_else(|| snapshot.clone()),
            ),
            generation: generation.clone(),
            catalog_name: catalog
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid catalog path")?
                .into(),
            settings_changes: changes,
            launch_confirmed: false,
        };
        self.save(&record)?;
        closed()?;
        profiles::write_private_file(&settings_path, installed.as_bytes())?;
        record.phase = LayoutPhase::SettingsApplied;
        self.save(&record)?;
        closed()?;
        record.phase = LayoutPhase::Committed;
        self.save(&record)?;
        Ok(record)
    }

    /// Recovery selects the untouched legacy folder and reverses only still-owned keys.
    /// Shared chats, snapshots and catalog/generation bytes are retained for recovery.
    pub fn rollback(
        &self,
        profile_id: &str,
        closed: impl Fn() -> Result<()>,
    ) -> Result<LayoutRecord> {
        let _lock = self.lock(profile_id)?;
        closed()?;
        let mut record = self
            .record(profile_id)?
            .ok_or("This profile has no layout migration")?;
        if record.phase == LayoutPhase::RolledBack {
            return Ok(record);
        }
        let legacy = self
            .data_root
            .join("profiles")
            .join(profile_id)
            .join("vscode-extensions");
        if !legacy.exists() {
            let migrations = crate::migration::MigrationStore::open(&self.data_root)?;
            let backup = record.legacy_snapshot.as_ref().unwrap_or(&record.snapshot);
            migrations.verify(backup)?;
            let source = migrations.snapshot_data(backup)?.join("vscode-extensions");
            if files::tree_manifest(&source)?
                .iter()
                .any(|(_, kind)| kind.starts_with("link:"))
            {
                return Err("Linked legacy extensions need full backup restore".into());
            }
            files::copy_tree(&source, &legacy)?;
        }
        files::root(&files::checked(&self.data_root, &legacy, false)?)?;
        let settings_path = self.settings_path(profile_id)?;
        let original = self.settings(&settings_path)?;
        let restored = managed_settings::rollback(&original, &record.settings_changes)?;
        closed()?;
        if self.settings(&settings_path)? != original {
            return Err(
                "Editor settings changed during rollback; retry with this profile closed".into(),
            );
        }
        record.phase = LayoutPhase::RollbackPending;
        self.save(&record)?;
        profiles::write_private_file(&settings_path, restored.as_bytes())?;
        record.phase = LayoutPhase::RolledBack;
        self.save(&record)?;
        Ok(record)
    }

    pub(crate) fn restore_settings(&self, profile_id: &str, staged: &Path) -> Result<()> {
        if let Some(record) = self.record(profile_id)? {
            let path = staged.join("vscode-user-data/User/settings.json");
            if path.exists() {
                let text = fs::read_to_string(files::regular(staged, &path)?)
                    .map_err(|_| "Cannot read restored settings")?;
                let restored = managed_settings::rollback(&text, &record.settings_changes)?;
                profiles::write_private_file(&path, restored.as_bytes())?;
            }
        }
        Ok(())
    }
    pub(crate) fn select_restored_legacy(&self, profile_id: &str) -> Result<()> {
        let _lock = self.lock(profile_id)?;
        if let Some(mut record) = self.record(profile_id)? {
            record.phase = LayoutPhase::RolledBack;
            self.save(&record)?;
        }
        Ok(())
    }
    pub(crate) fn confirm_launch(&self, profile_id: &str) -> Result<()> {
        let _lock = self.lock(profile_id)?;
        if let Some(mut record) = self.record(profile_id)? {
            if record.phase != LayoutPhase::Committed {
                return Err("Shared storage is not committed".into());
            }
            self.selected_catalog(profile_id)?;
            record.launch_confirmed = true;
            self.save(&record)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn fixture() -> (tempfile::TempDir, LayoutStore, SnapshotRecord, Generation) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("app");
        fs::create_dir(&root).unwrap();
        let profile = Uuid::new_v4().to_string();
        let base = root.join("profiles").join(&profile);
        fs::create_dir_all(base.join("vscode-extensions")).unwrap();
        fs::create_dir_all(base.join("vscode-user-data/User")).unwrap();
        fs::write(
            base.join("vscode-user-data/User/settings.json"),
            "{\n // private theme\n \"workbench.colorTheme\": \"Mine\"\n}\n",
        )
        .unwrap();
        fs::write(base.join("vscode-extensions/legacy.txt"), "legacy").unwrap();
        let mut sources = Vec::new();
        for (publisher, name) in [("openai", "chatgpt"), ("russeldanielpaul", "multi-codex")] {
            let package = temp.path().join(name);
            fs::create_dir(&package).unwrap();
            fs::write(
                package.join("package.json"),
                serde_json::to_vec(
                    &serde_json::json!({"publisher":publisher,"name":name,"version":"1.0.0"}),
                )
                .unwrap(),
            )
            .unwrap();
            sources.push(package);
        }
        let generation = ExtensionStore::open(&root)
            .unwrap()
            .publish(&sources, || Ok(()))
            .unwrap();
        let snapshot = crate::migration::MigrationStore::open(&root)
            .unwrap()
            .prepare(&profile, b"{}", || Ok(()))
            .unwrap();
        let layout = LayoutStore::open(&root).unwrap();
        (temp, layout, snapshot, generation)
    }
    fn desired() -> BTreeMap<String, Value> {
        BTreeMap::from([
            ("chatgpt.openOnStartup".into(), Value::Bool(true)),
            ("extensions.autoUpdate".into(), Value::Bool(false)),
        ])
    }
    fn cleanup(path: &Path) {
        let metadata = fs::symlink_metadata(path).unwrap();
        if files::is_link(&metadata) {
            return;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                path,
                fs::Permissions::from_mode(if metadata.is_dir() { 0o700 } else { 0o600 }),
            )
            .unwrap();
        }
        #[cfg(windows)]
        {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                cleanup(&entry.unwrap().path());
            }
        }
    }
    #[test]
    fn selection_and_rollback_preserve_legacy_and_later_user_settings() {
        let (_temp, layout, snapshot, generation) = fixture();
        let legacy = layout
            .data_root
            .join("profiles")
            .join(&snapshot.profile_id)
            .join("vscode-extensions");
        let before = files::tree_manifest(&legacy).unwrap();
        let record = layout
            .commit(&snapshot, &generation, &desired(), || Ok(()))
            .unwrap();
        assert_eq!(record.phase, LayoutPhase::Committed);
        assert!(layout
            .selected_catalog(&snapshot.profile_id)
            .unwrap()
            .is_some());
        let settings = layout.settings_path(&snapshot.profile_id).unwrap();
        let edited = layout
            .settings(&settings)
            .unwrap()
            .replace("Mine", "Changed later");
        fs::write(&settings, edited).unwrap();
        layout.rollback(&snapshot.profile_id, || Ok(())).unwrap();
        assert!(layout
            .selected_catalog(&snapshot.profile_id)
            .unwrap()
            .is_none());
        let restored = layout.settings(&settings).unwrap();
        assert!(restored.contains("Changed later"));
        assert!(restored.contains("// private theme"));
        assert!(!restored.contains("chatgpt.openOnStartup"));
        assert_eq!(before, files::tree_manifest(&legacy).unwrap());
        crate::migration::MigrationStore::open(&layout.data_root)
            .unwrap()
            .verify(&snapshot)
            .unwrap();
        cleanup(&layout.data_root);
    }
    #[test]
    fn interrupted_settings_commit_blocks_launch_until_recovery() {
        let (_temp, layout, snapshot, generation) = fixture();
        let calls = Cell::new(0);
        let outcome = layout.commit(&snapshot, &generation, &desired(), || {
            calls.set(calls.get() + 1);
            if calls.get() == 4 {
                Err("Injected interruption before selector commit".into())
            } else {
                Ok(())
            }
        });
        assert!(outcome.is_err());
        assert_eq!(
            layout.record(&snapshot.profile_id).unwrap().unwrap().phase,
            LayoutPhase::SettingsApplied
        );
        assert!(layout.selected_catalog(&snapshot.profile_id).is_err());
        layout.rollback(&snapshot.profile_id, || Ok(())).unwrap();
        assert!(layout
            .selected_catalog(&snapshot.profile_id)
            .unwrap()
            .is_none());
        cleanup(&layout.data_root);
    }
    #[test]
    fn changed_source_after_snapshot_cannot_authorize_migration() {
        let (_temp, layout, snapshot, generation) = fixture();
        fs::write(
            layout
                .data_root
                .join("profiles")
                .join(&snapshot.profile_id)
                .join("new-data"),
            "new work",
        )
        .unwrap();
        assert!(layout
            .commit(&snapshot, &generation, &desired(), || Ok(()))
            .is_err());
        assert!(layout.record(&snapshot.profile_id).unwrap().is_none());
        cleanup(&layout.data_root);
    }
    #[test]
    fn catalog_repair_retains_original_backup_and_cleanup_keeps_rollback_available() {
        let (_temp, layout, snapshot, generation) = fixture();
        let record = layout
            .commit(&snapshot, &generation, &desired(), || Ok(()))
            .unwrap();
        let catalog = layout.catalog(&record).unwrap();
        let codex = generation
            .packages
            .iter()
            .find(|package| package.id == "openai.chatgpt")
            .unwrap();
        let link = catalog.join(&codex.directory);
        #[cfg(unix)]
        fs::remove_file(&link).unwrap();
        #[cfg(windows)]
        junction::delete(&link).unwrap();
        assert!(layout.selected_catalog(&snapshot.profile_id).is_err());
        let backups = crate::migration::MigrationStore::open(&layout.data_root).unwrap();
        let fresh = backups
            .prepare(&snapshot.profile_id, b"{}", || Ok(()))
            .unwrap();
        let repaired = layout
            .commit(&fresh, &generation, &desired(), || Ok(()))
            .unwrap();
        assert_ne!(record.catalog_name, repaired.catalog_name);
        assert_eq!(
            repaired.legacy_snapshot.unwrap().snapshot_id,
            snapshot.snapshot_id
        );
        assert!(layout
            .selected_catalog(&snapshot.profile_id)
            .unwrap()
            .is_some());
        assert!(crate::legacy_cleanup::preview(&layout.data_root, &snapshot.profile_id).is_err());
        layout.confirm_launch(&snapshot.profile_id).unwrap();
        let preview =
            crate::legacy_cleanup::preview(&layout.data_root, &snapshot.profile_id).unwrap();
        assert!(preview.bytes > 0);
        crate::legacy_cleanup::clear(
            &layout.data_root,
            &snapshot.profile_id,
            &preview.token,
            || Ok(()),
        )
        .unwrap();
        let legacy = layout
            .data_root
            .join("profiles")
            .join(&snapshot.profile_id)
            .join("vscode-extensions");
        assert!(!legacy.exists());
        layout.rollback(&snapshot.profile_id, || Ok(())).unwrap();
        assert_eq!(
            fs::read_to_string(legacy.join("legacy.txt")).unwrap(),
            "legacy"
        );
        assert!(layout
            .selected_catalog(&snapshot.profile_id)
            .unwrap()
            .is_none());
        backups.verify(&snapshot).unwrap();
        cleanup(&layout.data_root);
    }
}
