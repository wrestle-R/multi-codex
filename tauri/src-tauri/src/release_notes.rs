use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{Manager, State};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseStatus {
    version: String,
    show_notes: bool,
}

#[derive(Serialize, Deserialize)]
struct Receipt {
    version: String,
}

fn version_parts(version: &str) -> Option<Vec<u64>> {
    let parts: Vec<_> = version
        .split('.')
        .map(str::parse::<u64>)
        .collect::<Result<_, _>>()
        .ok()?;
    (parts.len() == 3).then_some(parts)
}

fn acknowledge(root: &Path, version: &str) -> Result<(), String> {
    let path = root.join("release-notes.json");
    multi_codex_core::managed_files::checked(root, &path, true)?;
    let mut temporary = tempfile::NamedTempFile::new_in(root)
        .map_err(|_| "Could not save release notes receipt")?;
    multi_codex_core::file_security::protect(temporary.path(), false)?;
    serde_json::to_writer(
        temporary.as_file_mut(),
        &Receipt {
            version: version.into(),
        },
    )
    .map_err(|_| "Could not encode release notes receipt")?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|_| "Could not flush release notes receipt")?;
    temporary
        .persist(&path)
        .map_err(|_| "Could not commit release notes receipt")?;
    multi_codex_core::file_security::sync_directory(root)
}

fn status(root: &Path, version: &str, existing_accounts: bool) -> Result<ReleaseStatus, String> {
    let path = root.join("release-notes.json");
    multi_codex_core::managed_files::checked(root, &path, true)?;
    let previous = match std::fs::read(&path) {
        Ok(bytes) => Some(
            serde_json::from_slice::<Receipt>(&bytes)
                .map_err(|_| "Release notes receipt is unreadable")?,
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(_) => return Err("Release notes receipt is unavailable".into()),
    };
    let show_notes = match previous {
        Some(receipt) => match (version_parts(version), version_parts(&receipt.version)) {
            (Some(current), Some(seen)) => current > seen,
            _ => false,
        },
        None if existing_accounts => true,
        None => {
            acknowledge(root, version)?;
            false
        }
    };
    Ok(ReleaseStatus {
        version: version.into(),
        show_notes,
    })
}

#[tauri::command]
pub fn get_app_release(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
) -> Result<ReleaseStatus, String> {
    status(
        state.service.data_directory(),
        &app.package_info().version.to_string(),
        !state.service.list_profiles()?.is_empty()
            || std::fs::read(state.service.data_directory().join("executables.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
                .is_some_and(|settings| settings["onboardingCompleted"] == true),
    )
}

#[tauri::command]
pub fn acknowledge_app_release(
    app: tauri::AppHandle,
    state: State<'_, crate::AppState>,
) -> Result<(), String> {
    acknowledge(
        state.service.data_directory(),
        &app.package_info().version.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn existing_install_shows_notes_until_acknowledged() {
        let root = tempfile::tempdir().unwrap();
        assert!(status(root.path(), "1.7.0", true).unwrap().show_notes);
        assert!(status(root.path(), "1.7.0", true).unwrap().show_notes);
        acknowledge(root.path(), "1.7.0").unwrap();
        assert!(!status(root.path(), "1.7.0", true).unwrap().show_notes);
    }
    #[test]
    fn fresh_install_waits_for_a_newer_release_without_downgrade_prompts() {
        let root = tempfile::tempdir().unwrap();
        assert!(!status(root.path(), "1.7.0", false).unwrap().show_notes);
        assert!(status(root.path(), "1.8.0", true).unwrap().show_notes);
        assert!(!status(root.path(), "1.6.1", true).unwrap().show_notes);
        assert!(status(root.path(), "1.10.0", true).unwrap().show_notes);
    }
    #[test]
    fn malformed_receipt_is_not_silently_acknowledged() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("release-notes.json"), "broken").unwrap();
        assert!(status(root.path(), "1.7.0", true).is_err());
        assert_eq!(
            std::fs::read_to_string(root.path().join("release-notes.json")).unwrap(),
            "broken"
        );
    }
}
