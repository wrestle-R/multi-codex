//! Explicit extension-only cleanup after verified native activation and backup.
use crate::{
    managed_files as files,
    migration::MigrationStore,
    profile_layout::{LayoutPhase, LayoutStore},
    profiles::{self, Result},
};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use uuid::Uuid;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Preview {
    pub token: String,
    pub profile_id: String,
    pub bytes: u64,
    pub digest: String,
    pub snapshot_id: String,
}
fn assess(data: &Path, id: &str) -> Result<Preview> {
    let layout = LayoutStore::open(data)?;
    let record = layout
        .record(id)?
        .ok_or("Enable sharing and verify a native launch before clearing original extensions")?;
    if record.phase != LayoutPhase::Committed || !record.launch_confirmed {
        return Err(
            "Open Codex successfully with this shared account before clearing original extensions"
                .into(),
        );
    }
    layout.selected_catalog(id)?;
    let migrations = MigrationStore::open(data)?;
    let backup = record.legacy_snapshot.as_ref().unwrap_or(&record.snapshot);
    migrations.verify(backup)?;
    let original = data.join("profiles").join(id).join("vscode-extensions");
    let expected = migrations.snapshot_data(backup)?.join("vscode-extensions");
    let rows = files::tree_manifest(&files::checked(data, &original, false)?)?;
    if rows.iter().any(|(_, kind)| kind.starts_with("link:")) {
        return Err("Linked legacy extensions require a separate cleanup policy".into());
    }
    if rows != files::tree_manifest(&expected)? {
        return Err("Original extensions changed after backup; prepare a new migration backup before clearing them".into());
    }
    let bytes = rows
        .iter()
        .filter(|(_, kind)| kind.starts_with("file:"))
        .try_fold(0u64, |sum, (path, _)| {
            fs::metadata(original.join(path))
                .map(|m| sum.saturating_add(m.len()))
                .map_err(|_| "Cannot measure original extensions")
        })?;
    Ok(Preview {
        token: Uuid::new_v4().to_string(),
        profile_id: id.into(),
        bytes,
        digest: files::manifest_digest(&rows)?,
        snapshot_id: backup.snapshot_id.clone(),
    })
}
pub fn preview(data: &Path, id: &str) -> Result<Preview> {
    let result = assess(data, id)?;
    let root = data.join("migrations/legacy-previews");
    files::directory(data, &root)?;
    profiles::write_private_file(
        &root.join(format!("{}.json", result.token)),
        &serde_json::to_vec(&result).map_err(|_| "Cannot encode extension cleanup preview")?,
    )?;
    Ok(result)
}
pub fn clear(data: &Path, id: &str, token: &str, closed: impl Fn() -> Result<()>) -> Result<u64> {
    closed()?;
    if !Uuid::parse_str(token).is_ok_and(|id| id.to_string() == token) {
        return Err("Invalid extension cleanup preview".into());
    }
    let path = files::regular(
        data,
        &data
            .join("migrations/legacy-previews")
            .join(format!("{token}.json")),
    )?;
    let prior: Preview = serde_json::from_slice(
        &fs::read(&path).map_err(|_| "Cannot read extension cleanup preview")?,
    )
    .map_err(|_| "Invalid extension cleanup preview")?;
    if prior.profile_id != id {
        return Err("Extension cleanup preview belongs to another profile".into());
    }
    let current = assess(data, id)?;
    if prior.digest != current.digest
        || prior.snapshot_id != current.snapshot_id
        || prior.bytes != current.bytes
    {
        return Err("Original extensions changed. Check the preview again".into());
    }
    closed()?;
    let original = files::checked(
        data,
        &data.join("profiles").join(id).join("vscode-extensions"),
        false,
    )?;
    fs::remove_dir_all(original)
        .map_err(|_| "Cannot clear original extensions; verified backup remains retained")?;
    fs::remove_file(path)
        .map_err(|_| "Extensions were cleared, but their preview could not be removed")?;
    Ok(current.bytes)
}
