//! One no-follow disk inventory. Shared allocations have one stable owner.
use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Default)]
pub struct Inventory {
    entries: BTreeMap<(u64, u64), Entry>,
}
struct Entry {
    bytes: u64,
    links: u64,
    file: bool,
    paths: Vec<PathBuf>,
}

#[cfg(unix)]
fn allocation(_path: &Path, metadata: &fs::Metadata) -> io::Result<((u64, u64), u64, u64)> {
    use std::os::unix::fs::MetadataExt;
    Ok((
        (metadata.dev(), metadata.ino()),
        metadata.blocks().saturating_mul(512),
        metadata.nlink(),
    ))
}
#[cfg(windows)]
fn allocation(path: &Path, _metadata: &fs::Metadata) -> io::Result<((u64, u64), u64, u64)> {
    use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::Storage::FileSystem::*;
    let file = fs::OpenOptions::new()
        .access_mode(0)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)?;
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    let mut standard: FILE_STANDARD_INFO = unsafe { std::mem::zeroed() };
    unsafe {
        if GetFileInformationByHandle(file.as_raw_handle(), &mut info) == 0
            || GetFileInformationByHandleEx(
                file.as_raw_handle(),
                FileStandardInfo,
                (&mut standard as *mut FILE_STANDARD_INFO).cast(),
                std::mem::size_of::<FILE_STANDARD_INFO>() as u32,
            ) == 0
        {
            return Err(io::Error::last_os_error());
        }
    }
    Ok((
        (
            u64::from(info.dwVolumeSerialNumber),
            (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
        ),
        standard.AllocationSize.max(0) as u64,
        u64::from(info.nNumberOfLinks),
    ))
}

impl Inventory {
    pub fn scan(root: &Path) -> io::Result<Self> {
        let mut inventory = Self::default();
        // Shared payloads get attribution before private hardlinked registries.
        inventory.visit(root)?;
        Ok(inventory)
    }
    fn visit(&mut self, path: &Path) -> io::Result<()> {
        let metadata = match fs::symlink_metadata(path) {
            Ok(value) => value,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(e) => return Err(e),
        };
        let (id, bytes, links) = allocation(path, &metadata)?;
        self.entries
            .entry(id)
            .or_insert_with(|| Entry {
                bytes,
                links,
                file: metadata.is_file(),
                paths: Vec::new(),
            })
            .paths
            .push(path.to_path_buf());
        if metadata.is_dir() && !crate::managed_files::is_link(&metadata) {
            let directory = match fs::read_dir(path) {
                Ok(directory) => directory,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(error),
            };
            let mut children = directory
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<io::Result<Vec<_>>>()?;
            children.sort_by_key(|path| {
                (
                    match path.file_name().and_then(|name| name.to_str()) {
                        Some("shared-vscode") => 0,
                        Some("shared-chats") => 1,
                        Some("migrations") => 2,
                        _ => 3,
                    },
                    path.clone(),
                )
            });
            for child in children {
                self.visit(&child)?;
            }
        }
        Ok(())
    }
    pub fn total(&self) -> u64 {
        self.entries.values().map(|entry| entry.bytes).sum()
    }
    /// Nested breakdowns use the same attribution as the top-level total.
    pub fn under(&self, root: &Path) -> u64 {
        self.entries
            .values()
            .filter(|entry| entry.paths[0].starts_with(root))
            .map(|entry| entry.bytes)
            .sum()
    }
    pub fn reclaimable(&self, roots: &[PathBuf]) -> u64 {
        self.entries
            .values()
            .filter(|entry| {
                (!entry.file || entry.links <= entry.paths.len() as u64)
                    && entry
                        .paths
                        .iter()
                        .all(|path| roots.iter().any(|root| path.starts_with(root)))
            })
            .map(|entry| entry.bytes)
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hardlinks_are_counted_once_and_cache_retained_elsewhere_is_not_clearable() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        fs::create_dir_all(root.join("shared-vscode")).unwrap();
        fs::create_dir_all(root.join("profiles/a/cache")).unwrap();
        fs::write(root.join("shared-vscode/package"), vec![7; 8192]).unwrap();
        fs::hard_link(
            root.join("shared-vscode/package"),
            root.join("profiles/a/cache/package"),
        )
        .unwrap();
        let inventory = Inventory::scan(root).unwrap();
        let shared = inventory.under(&root.join("shared-vscode"));
        assert!(shared >= 8192);
        assert!(inventory.under(&root.join("profiles/a/cache")) < 8192);
        fs::write(root.join("profiles/a/cache/disposable"), vec![8; 4096]).unwrap();
        let inventory = Inventory::scan(root).unwrap();
        assert!(inventory.reclaimable(&[root.join("profiles/a/cache")]) >= 4096);
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let expected: u64 = [
                root,
                &root.join("shared-vscode"),
                &root.join("profiles"),
                &root.join("profiles/a"),
                &root.join("profiles/a/cache"),
                &root.join("shared-vscode/package"),
                &root.join("profiles/a/cache/disposable"),
            ]
            .iter()
            .map(|path| path.symlink_metadata().unwrap().blocks() * 512)
            .sum();
            assert_eq!(inventory.total(), expected);
            #[cfg(target_os = "linux")]
            {
                let du = std::process::Command::new("du")
                    .args(["-s", "-B1"])
                    .arg(root)
                    .output()
                    .unwrap();
                assert!(du.status.success());
                let measured: u64 = String::from_utf8(du.stdout)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse()
                    .unwrap();
                assert_eq!(inventory.total(), measured);
            }
        }
    }
    #[cfg(unix)]
    #[test]
    fn sparse_files_and_external_symlinks_match_allocated_blocks() {
        use std::os::unix::fs::{symlink, MetadataExt};
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let file = fs::File::create(root.join("sparse")).unwrap();
        file.set_len(1024 * 1024 * 1024).unwrap();
        symlink("/", root.join("outside")).unwrap();
        let inventory = Inventory::scan(root).unwrap();
        assert_eq!(
            inventory.total(),
            root.symlink_metadata().unwrap().blocks() * 512
                + root.join("sparse").symlink_metadata().unwrap().blocks() * 512
                + root.join("outside").symlink_metadata().unwrap().blocks() * 512
        );
        assert!(inventory.total() < 1024 * 1024);
    }
}
