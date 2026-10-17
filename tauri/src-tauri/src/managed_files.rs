//! Owned storage boundaries and verified, durable copies used by migration.
use crate::profiles::{self, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

pub fn is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        // Junctions and other reparse points must not masquerade as directories.
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

pub fn root(path: &Path) -> Result<()> {
    if !path.is_absolute() || path.components().any(|p| matches!(p, Component::ParentDir)) {
        return Err("Storage root must be an absolute owned directory".into());
    }
    let metadata = fs::symlink_metadata(path).map_err(|_| "Storage root is unavailable")?;
    if is_link(&metadata) || !metadata.is_dir() {
        return Err("Storage root cannot be a link".into());
    }
    Ok(())
}

/// Check every entry below an established root without following links.
/// Missing leaves are allowed only for creation; an existing invalid entry is never replaced.
pub fn checked(root_path: &Path, path: &Path, missing: bool) -> Result<PathBuf> {
    root(root_path)?;
    if path == root_path || !path.starts_with(root_path) {
        return Err("Storage path must remain inside its owned root".into());
    }
    let mut current = root_path.to_path_buf();
    let parts: Vec<_> = path
        .strip_prefix(root_path)
        .map_err(|_| "Invalid storage path")?
        .components()
        .collect();
    for (index, part) in parts.iter().enumerate() {
        let Component::Normal(name) = part else {
            return Err("Invalid storage path".into());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if is_link(&metadata) || (index + 1 < parts.len() && !metadata.is_dir()) {
                    return Err("Storage paths cannot follow linked or invalid entries".into());
                }
            }
            Err(error) if missing && error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Storage path is unavailable".into()),
        }
    }
    Ok(current)
}

pub fn directory(root_path: &Path, path: &Path) -> Result<()> {
    checked(root_path, path, true)?;
    let mut current = root_path.to_path_buf();
    for part in path
        .strip_prefix(root_path)
        .map_err(|_| "Invalid storage path")?
        .components()
    {
        let Component::Normal(name) = part else {
            return Err("Invalid storage path".into());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if !is_link(&metadata) && metadata.is_dir() => {}
            Ok(_) => return Err("Storage directories cannot be linked or invalid".into()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|_| "Could not create storage directory")?;
                crate::file_security::sync_directory(
                    current.parent().ok_or("Missing storage parent")?,
                )?;
            }
            Err(_) => return Err("Could not inspect storage directory".into()),
        }
        profiles::set_owner_only_dir(&current)?;
    }
    Ok(())
}

pub fn regular(root_path: &Path, path: &Path) -> Result<PathBuf> {
    let path = checked(root_path, path, false)?;
    if !fs::symlink_metadata(&path)
        .map_err(|_| "Storage file unavailable")?
        .is_file()
    {
        return Err("Expected a regular storage file".into());
    }
    Ok(path)
}

pub fn digest(path: &Path) -> Result<String> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "Could not inspect file for verification")?;
    if !metadata.is_file() || is_link(&metadata) {
        return Err("Only regular files can be hashed".into());
    }
    let mut file = File::open(path).map_err(|_| "Could not read file for verification")?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 128 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|_| "Could not verify file content")?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

pub fn link_target(path: &Path) -> Result<PathBuf> {
    #[cfg(windows)]
    if junction::exists(path).unwrap_or(false) {
        return junction::get_target(path).map_err(|_| "Could not inspect storage junction".into());
    }
    fs::read_link(path).map_err(|_| "Could not inspect storage link".into())
}

/// The destination must be new. Copy never follows links or deletes/replaces a source.
pub fn copy_tree(source: &Path, destination: &Path) -> Result<()> {
    if fs::symlink_metadata(destination).is_ok() {
        return Err("Snapshot destination already exists; it will not be overwritten".into());
    }
    let metadata = fs::symlink_metadata(source).map_err(|_| "Could not inspect source data")?;
    if is_link(&metadata) {
        copy_link(source, destination)?;
    } else if metadata.is_dir() {
        fs::create_dir(destination).map_err(|_| "Could not create snapshot directory")?;
        profiles::set_owner_only_dir(destination)?;
        for entry in fs::read_dir(source).map_err(|_| "Could not read source directory")? {
            let entry = entry.map_err(|_| "Could not read source entry")?;
            copy_tree(&entry.path(), &destination.join(entry.file_name()))?;
        }
        crate::file_security::sync_directory(destination)?;
    } else if metadata.is_file() {
        fs::copy(source, destination)
            .map_err(|_| "Could not copy source data; originals have been kept")?;
        // Preserve owner execution/read-only behavior while keeping backup data private.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                destination,
                fs::Permissions::from_mode(metadata.permissions().mode() & 0o700),
            )
            .map_err(|_| "Could not protect snapshot file")?;
        }
        #[cfg(windows)]
        {
            crate::file_security::protect(destination, false)?;
        }
        #[cfg(unix)]
        File::open(destination)
            .and_then(|file| file.sync_all())
            .map_err(|_| "Could not persist snapshot file")?;
        #[cfg(windows)]
        {
            let mut writable = fs::metadata(destination)
                .map_err(|_| "Could not inspect copied file attributes")?
                .permissions();
            writable.set_readonly(false);
            fs::set_permissions(destination, writable)
                .map_err(|_| "Could not flush a read-only source copy")?;
            fs::OpenOptions::new()
                .write(true)
                .open(destination)
                .and_then(|file| file.sync_all())
                .map_err(|_| "Could not persist snapshot file")?;
            fs::set_permissions(destination, metadata.permissions())
                .map_err(|_| "Could not preserve snapshot file attributes")?;
        }
    } else {
        return Err("Close all sessions before copying transient profile entries".into());
    }
    Ok(())
}

#[cfg(unix)]
fn copy_link(source: &Path, destination: &Path) -> Result<()> {
    std::os::unix::fs::symlink(link_target(source)?, destination)
        .map_err(|_| "Could not preserve snapshot link".into())
}

#[cfg(windows)]
fn copy_link(source: &Path, destination: &Path) -> Result<()> {
    let target = link_target(source)?;
    if junction::exists(source).unwrap_or(false) {
        return junction::create(target, destination)
            .map_err(|_| "Could not preserve snapshot junction".into());
    }
    if source.is_dir() {
        std::os::windows::fs::symlink_dir(target, destination)
    } else {
        std::os::windows::fs::symlink_file(target, destination)
    }
    .map_err(|_| "Could not preserve a Windows link; originals have been kept".into())
}

pub fn package_link(target: &Path, link: &Path) -> Result<()> {
    root(target)?;
    if fs::symlink_metadata(link).is_ok() {
        return Err("Extension catalog entry already exists".into());
    }
    #[cfg(unix)]
    let result = std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    let result = junction::create(target, link);
    result.map_err(|_| "Could not create the private extension catalog".into())
}

pub fn tree_manifest(root_path: &Path) -> Result<Vec<(PathBuf, String)>> {
    root(root_path)?;
    fn walk(root: &Path, path: &Path, rows: &mut Vec<(PathBuf, String)>) -> Result<()> {
        let metadata = fs::symlink_metadata(path).map_err(|_| "Could not inspect snapshot")?;
        let relative = path
            .strip_prefix(root)
            .map_err(|_| "Snapshot escaped its root")?
            .to_path_buf();
        if is_link(&metadata) {
            let target = link_target(path)?;
            rows.push((
                relative,
                format!(
                    "link:{}",
                    target.to_str().ok_or("Storage link is not valid Unicode")?
                ),
            ));
        } else if metadata.is_dir() {
            rows.push((relative, "directory".into()));
            for entry in fs::read_dir(path).map_err(|_| "Could not inspect snapshot")? {
                walk(
                    root,
                    &entry.map_err(|_| "Could not inspect snapshot")?.path(),
                    rows,
                )?;
            }
        } else if metadata.is_file() {
            #[cfg(unix)]
            let attributes = {
                use std::os::unix::fs::PermissionsExt;
                format!("{:o}", metadata.permissions().mode() & 0o700)
            };
            #[cfg(windows)]
            let attributes = format!("readonly{}", u8::from(metadata.permissions().readonly()));
            rows.push((relative, format!("file:{attributes}:{}", digest(path)?)));
        } else {
            return Err("A profile has transient entries; close its sessions first".into());
        }
        Ok(())
    }
    let mut rows = Vec::new();
    walk(root_path, root_path, &mut rows)?;
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(rows)
}

pub fn manifest_digest(rows: &[(PathBuf, String)]) -> Result<String> {
    let bytes = serde_json::to_vec(rows).map_err(|_| "Could not encode storage manifest")?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

/// Verify both the copy and that the source did not change while it was copied.
pub fn verified_snapshot(source: &Path, destination: &Path) -> Result<()> {
    if destination.starts_with(source) || source.starts_with(destination) {
        return Err("Snapshot roots must be disjoint".into());
    }
    root(source)?;
    root(destination.parent().ok_or("Snapshot has no parent")?)?;
    let marker = destination.with_extension("verified.json");
    if fs::symlink_metadata(&marker).is_ok() {
        return Err("Snapshot completion marker already exists".into());
    }
    let before = tree_manifest(source)?;
    copy_tree(source, destination)?;
    if before != tree_manifest(destination)? || before != tree_manifest(source)? {
        return Err("Profile data changed during backup. Originals are untouched; close its sessions and retry".into());
    }
    profiles::write_private_file(
        &marker,
        &serde_json::to_vec(&before).map_err(|_| "Could not record the verified backup")?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapshots_verify_content_empty_directories_and_links_without_following_them() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir_all(source.join("empty")).unwrap();
        fs::write(source.join("chat"), "complete context").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("chat", source.join("alias")).unwrap();
        let backup = temp.path().join("backup");
        verified_snapshot(&source, &backup).unwrap();
        assert_eq!(
            tree_manifest(&source).unwrap(),
            tree_manifest(&backup).unwrap()
        );
        fs::write(source.join("chat"), "new work").unwrap();
        assert_eq!(
            fs::read_to_string(backup.join("chat")).unwrap(),
            "complete context"
        );
        assert!(verified_snapshot(&source, &backup).is_err());
    }
    #[test]
    fn managed_files_reject_linked_ancestors_and_path_escape() {
        let temp = tempfile::tempdir().unwrap();
        let base = temp.path();
        directory(base, &base.join("private/child")).unwrap();
        assert!(directory(base, &base.join("private/../escape")).is_err());
        assert!(regular(base, base).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(base.join("private"), base.join("alias")).unwrap();
            assert!(directory(base, &base.join("alias/child")).is_err());
            assert!(regular(&base.join("alias"), &base.join("alias/child")).is_err());
        }
    }
    #[test]
    fn snapshots_cannot_be_written_inside_the_source() {
        let temp = tempfile::tempdir().unwrap();
        assert!(verified_snapshot(temp.path(), &temp.path().join("recursive")).is_err());
        assert!(!temp.path().join("recursive").exists());
    }
    #[cfg(unix)]
    #[test]
    fn snapshots_reject_live_sockets_without_a_verified_marker() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        fs::create_dir(&source).unwrap();
        let _socket = std::os::unix::net::UnixListener::bind(source.join("live.sock")).unwrap();
        let destination = temp.path().join("backup");
        assert!(verified_snapshot(&source, &destination).is_err());
        assert!(!destination.with_extension("verified.json").exists());
    }
}
