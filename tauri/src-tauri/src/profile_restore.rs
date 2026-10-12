//! Recoverable restore activation; the entire displaced profile remains retained.
use crate::{
    managed_files as files,
    migration::MigrationStore,
    profiles::{self, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreRecord {
    pub schema: u32,
    pub profile_id: String,
    pub operation_id: String,
    pub phase: String,
    pub digest: String,
}
fn journal(data: &Path, id: &str) -> Result<PathBuf> {
    if !Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id) {
        return Err("Invalid restore profile".into());
    }
    files::directory(data, &data.join("migrations/restore-journals"))?;
    files::checked(
        data,
        &data
            .join("migrations/restore-journals")
            .join(format!("{id}.json")),
        true,
    )
}
pub fn record(data: &Path, id: &str) -> Result<Option<RestoreRecord>> {
    let path = journal(data, id)?;
    if !path.exists() {
        return Ok(None);
    }
    let value: RestoreRecord = serde_json::from_slice(
        &fs::read(files::regular(data, &path)?).map_err(|_| "Cannot read restore journal")?,
    )
    .map_err(|_| "Invalid restore journal")?;
    if value.schema != 1
        || value.profile_id != id
        || !Uuid::parse_str(&value.operation_id)
            .is_ok_and(|id| id.to_string() == value.operation_id)
        || !["prepared", "displaced", "installed", "complete"].contains(&value.phase.as_str())
    {
        return Err("Invalid restore journal identity or phase".into());
    }
    Ok(Some(value))
}
pub fn ensure_ready(data: &Path, id: &str) -> Result<()> {
    if record(data, id)?.is_some_and(|record| record.phase != "complete") {
        return Err("Profile restore was interrupted. Close this profile and finish its restore before launching".into());
    }
    Ok(())
}
fn save(data: &Path, record: &RestoreRecord) -> Result<()> {
    profiles::write_private_file(
        &journal(data, &record.profile_id)?,
        &serde_json::to_vec(record).map_err(|_| "Cannot encode restore journal")?,
    )
}
pub fn activate(data: &Path, id: &str, closed: impl Fn() -> Result<()>) -> Result<RestoreRecord> {
    let path = journal(data, id)?.with_extension("lock");
    let _lock = crate::file_security::exclusive_lock(
        crate::file_security::lock_file(&path)?,
        "Another profile restore is running",
    )?;
    closed()?;
    let restores = data.join("migrations/restores");
    files::directory(data, &restores)?;
    let mut operation =
        if let Some(record) = record(data, id)?.filter(|record| record.phase != "complete") {
            record
        } else {
            let operation_id = Uuid::new_v4().to_string();
            let staged = restores.join(&operation_id);
            MigrationStore::open(data)?.stage_restore(id, &staged, &closed)?;
            let current = data.join("profiles").join(id);
            // Preserve the current login. Restore does not reset or overwrite keychain
            // credentials, and an old filesystem backup cannot roll back auth refresh.
            let auth = current.join("codex-home/auth.json");
            let signed_out = current.join("codex-home/.multi-codex-signed-out");
            if signed_out.exists() {
                let marker = fs::read(files::regular(data, &signed_out)?)
                    .map_err(|_| "Cannot preserve current sign-out state")?;
                profiles::write_private_file(
                    &staged.join("codex-home/.multi-codex-signed-out"),
                    &marker,
                )?;
                let old_auth = staged.join("codex-home/auth.json");
                if old_auth.exists() {
                    fs::remove_file(files::regular(&staged, &old_auth)?)
                        .map_err(|_| "Cannot preserve signed-out restore")?;
                }
            } else if auth.exists() {
                let bytes = fs::read(files::regular(data, &auth)?)
                    .map_err(|_| "Cannot preserve current login")?;
                profiles::write_private_file(&staged.join("codex-home/auth.json"), &bytes)?;
            }
            crate::profile_layout::LayoutStore::open(data)?.restore_settings(id, &staged)?;
            let digest = files::manifest_digest(&files::tree_manifest(&staged)?)?;
            let record = RestoreRecord {
                schema: 1,
                profile_id: id.into(),
                operation_id,
                phase: "prepared".into(),
                digest,
            };
            save(data, &record)?;
            record
        };
    let staged = files::checked(data, &restores.join(&operation.operation_id), true)?;
    let current = files::checked(data, &data.join("profiles").join(id), true)?;
    let retained_root = data.join("migrations/displaced-profiles");
    files::directory(data, &retained_root)?;
    let retained = retained_root.join(&operation.operation_id);
    closed()?;
    if operation.phase == "prepared" {
        if !retained.exists() {
            files::root(&current)?;
            if files::manifest_digest(&files::tree_manifest(&staged)?)? != operation.digest {
                return Err(
                    "Staged restore failed verification; current profile is unchanged".into(),
                );
            }
            fs::rename(&current, &retained)
                .map_err(|_| "Cannot retain current profile for restore")?;
            crate::file_security::sync_directory(
                current.parent().ok_or("Invalid profile parent")?,
            )?;
            crate::file_security::sync_directory(&retained_root)?;
        } else if current.exists() {
            return Err(
                "Restore has conflicting current and retained profiles; recover before proceeding"
                    .into(),
            );
        }
        operation.phase = "displaced".into();
        save(data, &operation)?;
    }
    if operation.phase == "displaced" {
        if staged.exists() && !current.exists() {
            if files::manifest_digest(&files::tree_manifest(&staged)?)? != operation.digest {
                return Err(
                    "Restore staging changed; the displaced current profile remains retained"
                        .into(),
                );
            }
            closed()?;
            fs::rename(&staged, &current)
                .map_err(|_| "Cannot activate restored profile; current data remains retained")?;
            crate::file_security::sync_directory(
                current.parent().ok_or("Invalid profile parent")?,
            )?;
        } else if staged.exists() || !current.exists() {
            return Err("Incomplete restore directory transition requires recovery".into());
        }
        if files::manifest_digest(&files::tree_manifest(&current)?)? != operation.digest {
            return Err("Restored profile failed verification; displaced data is retained".into());
        }
        operation.phase = "installed".into();
        save(data, &operation)?;
    }
    closed()?;
    crate::profile_layout::LayoutStore::open(data)?.select_restored_legacy(id)?;
    operation.phase = "complete".into();
    save(data, &operation)?;
    Ok(operation)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn activation_retains_latest_work_and_login_and_recovers_after_rename() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("app");
        let id = Uuid::new_v4().to_string();
        let profile = data.join("profiles").join(&id);
        fs::create_dir_all(profile.join("codex-home")).unwrap();
        fs::create_dir(profile.join("vscode-extensions")).unwrap();
        fs::write(profile.join("codex-home/auth.json"), "old inert login").unwrap();
        fs::write(profile.join("old-history"), "complete history").unwrap();
        MigrationStore::open(&data)
            .unwrap()
            .prepare(&id, b"{}", || Ok(()))
            .unwrap();
        fs::write(
            profile.join("codex-home/auth.json"),
            "refreshed inert login",
        )
        .unwrap();
        fs::write(profile.join("later-work"), "keep latest work").unwrap();
        let result = activate(&data, &id, || Ok(())).unwrap();
        assert_eq!(result.phase, "complete");
        assert_eq!(
            fs::read_to_string(profile.join("codex-home/auth.json")).unwrap(),
            "refreshed inert login"
        );
        let retained = data
            .join("migrations/displaced-profiles")
            .join(&result.operation_id);
        assert_eq!(
            fs::read_to_string(retained.join("later-work")).unwrap(),
            "keep latest work"
        );
        assert!(!profile.join("later-work").exists());
        let mut pending = result.clone();
        pending.phase = "displaced".into();
        save(&data, &pending).unwrap();
        assert!(ensure_ready(&data, &id).is_err());
        let recovered = activate(&data, &id, || Ok(())).unwrap();
        assert_eq!(recovered.operation_id, result.operation_id);
        assert_eq!(recovered.phase, "complete");
        assert!(retained.join("later-work").exists());
        fs::remove_file(profile.join("codex-home/auth.json")).unwrap();
        fs::write(
            profile.join("codex-home/.multi-codex-signed-out"),
            "signed out",
        )
        .unwrap();
        activate(&data, &id, || Ok(())).unwrap();
        assert!(!profile.join("codex-home/auth.json").exists());
        assert!(profile.join("codex-home/.multi-codex-signed-out").exists());
    }
}
