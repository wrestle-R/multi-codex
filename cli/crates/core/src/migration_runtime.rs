//! Retain recognized stale runtime entries before a closed-profile snapshot.
//! Never follow links or remove entries: every original is moved into a journaled archive.
use crate::{
    managed_files as files,
    profiles::{self, Result},
};
use serde::Serialize;
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RetainedRuntime {
    schema: u32,
    profile_id: String,
    phase: String,
    entries: Vec<PathBuf>,
}
fn parent_is_owned(data: &Path, path: &Path) -> Result<()> {
    files::directory(data, path.parent().ok_or("Invalid runtime parent")?)
}
pub fn retain(data: &Path, id: &str) -> Result<usize> {
    if !uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id) {
        return Err("Invalid runtime profile".into());
    }
    let profile = data.join("profiles").join(id);
    files::root(&profile)?;
    let mut entries = Vec::new();
    // Exact known app-owned runtime leaves. Other special files remain refused.
    for relative in [
        "codex-home/ipc/ipc.sock",
        "vscode-user-data/SingletonLock",
        "vscode-user-data/SingletonSocket",
        "vscode-user-data/SingletonCookie",
        "desktop-data/SingletonLock",
        "desktop-data/SingletonSocket",
        "desktop-data/SingletonCookie",
    ] {
        let path = profile.join(relative);
        match fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Cannot inspect stale runtime entry".into()),
            Ok(metadata) => {
                #[cfg(unix)]
                let recognized = if relative.ends_with("ipc.sock") {
                    metadata.file_type().is_socket()
                } else {
                    metadata.file_type().is_symlink()
                };
                #[cfg(not(unix))]
                let recognized = metadata.file_type().is_symlink();
                if recognized {
                    parent_is_owned(data, &path)?;
                    entries.push(PathBuf::from(relative));
                }
            }
        }
    }
    let arguments = profile.join("codex-home/tmp/arg0");
    if arguments.exists() {
        files::checked(data, &arguments, false)?;
        for directory in
            fs::read_dir(&arguments).map_err(|_| "Cannot inspect Codex temporary arguments")?
        {
            let directory = directory.map_err(|_| "Cannot inspect Codex temporary arguments")?;
            let name = directory.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("codex-arg0")
                || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
            {
                continue;
            }
            let kind = directory
                .file_type()
                .map_err(|_| "Cannot inspect Codex temporary arguments")?;
            if !kind.is_dir() || kind.is_symlink() {
                continue;
            }
            files::checked(data, &directory.path(), false)?;
            for helper in [
                "codex-execve-wrapper",
                "codex-linux-sandbox",
                "apply_patch",
                "applypatch",
            ] {
                let path = directory.path().join(helper);
                if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
                    parent_is_owned(data, &path)?;
                    entries.push(
                        path.strip_prefix(&profile)
                            .map_err(|_| "Invalid temporary runtime path")?
                            .to_path_buf(),
                    );
                }
            }
        }
    }
    // Closed bridge windows retain credentials/history copies and absolute or
    // dangling links. Move each exact UUID directory intact; do not follow its
    // contents or discard them as cache. Both legacy and generation layouts use
    // these application-owned paths. The caller holds the closed-profile guards.
    for layout in [
        "bridge-runtime/windows",
        "vscode-user-data/User/globalStorage/russeldanielpaul.multi-codex/runtime/windows",
    ] {
        let windows = profile.join(layout);
        match fs::symlink_metadata(&windows) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err("Cannot inspect closed bridge windows".into()),
            Ok(_) => {
                files::checked(data, &windows, false)?;
            }
        }
        for entry in fs::read_dir(&windows).map_err(|_| "Cannot inspect closed bridge windows")? {
            let entry = entry.map_err(|_| "Cannot inspect closed bridge windows")?;
            let name = entry.file_name();
            if !name.to_str().is_some_and(|id| {
                uuid::Uuid::parse_str(id).is_ok_and(|parsed| parsed.to_string() == id)
            }) {
                continue;
            }
            let kind = entry
                .file_type()
                .map_err(|_| "Cannot inspect closed bridge window")?;
            if kind.is_dir() && !kind.is_symlink() {
                files::checked(data, &entry.path(), false)?;
                entries.push(
                    entry
                        .path()
                        .strip_prefix(&profile)
                        .map_err(|_| "Invalid bridge window path")?
                        .to_path_buf(),
                );
            }
        }
    }
    if entries.is_empty() {
        return Ok(0);
    }
    let retained = data
        .join("migrations/retained-runtime")
        .join(id)
        .join(uuid::Uuid::new_v4().to_string());
    files::directory(data, &retained)?;
    let mut record = RetainedRuntime {
        schema: 1,
        profile_id: id.into(),
        phase: "prepared".into(),
        entries,
    };
    let journal = retained.join("manifest.json");
    profiles::write_private_file(
        &journal,
        &serde_json::to_vec_pretty(&record).map_err(|_| "Cannot encode runtime retention")?,
    )?;
    for relative in &record.entries {
        let source = profile.join(relative);
        parent_is_owned(data, &source)?;
        let destination = retained.join(relative);
        files::directory(
            data,
            destination
                .parent()
                .ok_or("Invalid retained runtime path")?,
        )?;
        if destination.exists() || fs::symlink_metadata(&destination).is_ok() {
            return Err("Retained runtime destination already exists".into());
        }
        fs::rename(&source, &destination).map_err(|_| {
            "Cannot retain stale runtime entry; earlier entries remain in the recovery journal"
        })?;
        crate::file_security::sync_directory(source.parent().ok_or("Invalid runtime parent")?)?;
        crate::file_security::sync_directory(
            destination
                .parent()
                .ok_or("Invalid retained runtime parent")?,
        )?;
    }
    record.phase = "complete".into();
    profiles::write_private_file(
        &journal,
        &serde_json::to_vec_pretty(&record).map_err(|_| "Cannot encode runtime retention")?,
    )?;
    Ok(record.entries.len())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::{fs::symlink, net::UnixListener};
    #[test]
    fn stale_entries_are_retained_without_following_links_or_changing_history() {
        let temporary = tempfile::Builder::new()
            .prefix("mc")
            .tempdir_in("/tmp")
            .unwrap();
        let data = temporary.path();
        let id = uuid::Uuid::new_v4().to_string();
        let profile = data.join("profiles").join(&id);
        let home = profile.join("codex-home");
        fs::create_dir_all(home.join("ipc")).unwrap();
        fs::create_dir_all(home.join("tmp/arg0/codex-arg0Fixture")).unwrap();
        fs::write(home.join("auth.json"), "inert auth sentinel").unwrap();
        fs::write(home.join("history.jsonl"), "untouched history").unwrap();
        let external = temporary.path().join("external");
        fs::write(&external, "untouched external").unwrap();
        symlink(
            &external,
            home.join("tmp/arg0/codex-arg0Fixture/apply_patch"),
        )
        .unwrap();
        let socket = home.join("ipc/ipc.sock");
        let listener = UnixListener::bind(&socket).unwrap();
        drop(listener);
        assert_eq!(retain(data, &id).unwrap(), 2);
        assert_eq!(
            fs::read_to_string(home.join("auth.json")).unwrap(),
            "inert auth sentinel"
        );
        assert_eq!(
            fs::read_to_string(home.join("history.jsonl")).unwrap(),
            "untouched history"
        );
        assert_eq!(fs::read_to_string(external).unwrap(), "untouched external");
        let root = data.join("migrations/retained-runtime").join(&id);
        let retained = fs::read_dir(root).unwrap().next().unwrap().unwrap().path();
        assert!(fs::symlink_metadata(
            retained.join("codex-home/tmp/arg0/codex-arg0Fixture/apply_patch")
        )
        .unwrap()
        .file_type()
        .is_symlink());
        assert!(
            fs::symlink_metadata(retained.join("codex-home/ipc/ipc.sock"))
                .unwrap()
                .file_type()
                .is_socket()
        );
        assert_eq!(retain(data, &id).unwrap(), 0);
    }
    #[test]
    fn closed_bridge_windows_keep_credentials_and_dangling_links_in_the_archive() {
        let temporary = tempfile::tempdir().unwrap();
        let data = temporary.path();
        let id = uuid::Uuid::new_v4().to_string();
        let profile = data.join("profiles").join(&id);
        let external = temporary.path().join("external");
        fs::write(&external, "external sentinel").unwrap();
        let mut windows = Vec::new();
        for layout in [
            "bridge-runtime/windows",
            "vscode-user-data/User/globalStorage/russeldanielpaul.multi-codex/runtime/windows",
        ] {
            let window = profile.join(layout).join(uuid::Uuid::new_v4().to_string());
            fs::create_dir_all(&window).unwrap();
            fs::write(window.join("auth.json"), "private window sentinel").unwrap();
            symlink(&external, window.join("external-link")).unwrap();
            symlink(
                profile.join("codex-home/missing"),
                window.join("dangling-link"),
            )
            .unwrap();
            windows.push(window);
            fs::create_dir_all(profile.join(layout).join("unknown-window")).unwrap();
        }
        assert_eq!(retain(data, &id).unwrap(), 2);
        let archive = fs::read_dir(data.join("migrations/retained-runtime").join(&id))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        for window in windows {
            assert!(!window.exists());
            let retained = archive.join(window.strip_prefix(&profile).unwrap());
            assert_eq!(
                fs::read_to_string(retained.join("auth.json")).unwrap(),
                "private window sentinel"
            );
            assert_eq!(
                fs::read_link(retained.join("external-link")).unwrap(),
                external
            );
            assert!(fs::symlink_metadata(retained.join("dangling-link"))
                .unwrap()
                .file_type()
                .is_symlink());
            assert!(window.parent().unwrap().join("unknown-window").is_dir());
        }
        assert_eq!(fs::read_to_string(external).unwrap(), "external sentinel");
        assert_eq!(retain(data, &id).unwrap(), 0);
    }

    #[test]
    fn foreign_link_ancestors_are_refused() {
        let temporary = tempfile::tempdir().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let profile = temporary.path().join("profiles").join(&id);
        fs::create_dir_all(profile.join("codex-home/tmp")).unwrap();
        let foreign = tempfile::tempdir().unwrap();
        symlink(foreign.path(), profile.join("codex-home/tmp/arg0")).unwrap();
        assert!(retain(temporary.path(), &id).is_err());
    }
}
