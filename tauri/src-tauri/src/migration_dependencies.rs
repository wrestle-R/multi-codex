//! Freeze application-owned link targets without changing the original profile.
use crate::{
    managed_files as files,
    profiles::{self, Result},
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Dependency {
    pub link: PathBuf,
    pub target: PathBuf,
    pub digest: String,
    pub directory: bool,
}

pub fn inventory(data: &Path, source: &Path) -> Result<(Vec<Dependency>, Vec<PathBuf>, u64)> {
    let mut dependencies = Vec::new();
    let mut unresolved = Vec::new();
    let mut bytes = 0u64;
    fn walk(
        data: &Path,
        source: &Path,
        path: &Path,
        dependencies: &mut Vec<Dependency>,
        unresolved: &mut Vec<PathBuf>,
        bytes: &mut u64,
    ) -> Result<()> {
        let metadata =
            fs::symlink_metadata(path).map_err(|_| "Could not inspect migration dependency")?;
        if files::is_link(&metadata) {
            let relative = path
                .strip_prefix(source)
                .map_err(|_| "Invalid dependency link")?
                .to_path_buf();
            let target = fs::canonicalize(path);
            let Ok(target) = target else {
                unresolved.push(relative);
                return Ok(());
            };
            let owned = target.starts_with(source)
                || target.starts_with(data.join("shared-vscode/generations"))
                || target.starts_with(data.join("shared-chats/assets"));
            if !owned || target == source || path.starts_with(&target) {
                unresolved.push(relative);
                return Ok(());
            }
            let target = files::checked(data, &target, false)?;
            let directory = fs::metadata(&target)
                .map_err(|_| "Cannot inspect dependency target")?
                .is_dir();
            let digest = if directory {
                let manifest = files::tree_manifest(&target)?;
                // Nested links need their own ownership graph; never claim that a
                // shallow link copy freezes an unknown dependency.
                if manifest.iter().any(|(_, kind)| kind.starts_with("link:")) {
                    unresolved.push(relative);
                    return Ok(());
                }
                for (file, kind) in &manifest {
                    if kind.starts_with("file:") {
                        *bytes = bytes
                            .checked_add(
                                fs::metadata(target.join(file))
                                    .map_err(|_| "Cannot measure dependency")?
                                    .len(),
                            )
                            .ok_or("Dependency inventory is too large")?;
                    }
                }
                files::manifest_digest(&manifest)?
            } else {
                files::regular(data, &target)?;
                *bytes = bytes
                    .checked_add(
                        fs::metadata(&target)
                            .map_err(|_| "Cannot measure dependency")?
                            .len(),
                    )
                    .ok_or("Dependency inventory is too large")?;
                files::digest(&target)?
            };
            dependencies.push(Dependency {
                link: relative,
                target,
                digest,
                directory,
            });
        } else if metadata.is_dir() {
            for entry in fs::read_dir(path).map_err(|_| "Cannot inventory profile links")? {
                walk(
                    data,
                    source,
                    &entry.map_err(|_| "Cannot inventory profile link")?.path(),
                    dependencies,
                    unresolved,
                    bytes,
                )?;
            }
        }
        Ok(())
    }
    walk(
        data,
        source,
        source,
        &mut dependencies,
        &mut unresolved,
        &mut bytes,
    )?;
    dependencies.sort_by(|a, b| a.link.cmp(&b.link));
    unresolved.sort();
    Ok((dependencies, unresolved, bytes))
}

pub fn encode(dependencies: &[Dependency]) -> Result<Vec<u8>> {
    serde_json::to_vec(dependencies).map_err(|_| "Cannot encode dependency coverage".into())
}
pub fn freeze(data: &Path, snapshot: &Path, dependencies: &[Dependency]) -> Result<()> {
    let root = snapshot.join("dependencies");
    files::directory(snapshot, &root)?;
    for (index, dependency) in dependencies.iter().enumerate() {
        files::checked(data, &dependency.target, false)?;
        let copy = root.join(index.to_string());
        files::copy_tree(&dependency.target, &copy)?;
        verify_copy(&copy, dependency)?;
        verify_copy(&dependency.target, dependency)?;
    }
    profiles::write_private_file(&snapshot.join("dependencies.json"), &encode(dependencies)?)
}
fn verify_copy(copy: &Path, dependency: &Dependency) -> Result<()> {
    let actual = if dependency.directory {
        files::manifest_digest(&files::tree_manifest(copy)?)?
    } else {
        files::digest(copy)?
    };
    if actual != dependency.digest {
        return Err("A backed-up dependency changed or failed verification".into());
    }
    Ok(())
}
pub fn verify(snapshot: &Path, digest: &str) -> Result<Vec<Dependency>> {
    let manifest = files::regular(snapshot, &snapshot.join("dependencies.json"))?;
    if files::digest(&manifest)? != digest {
        return Err("Dependency coverage manifest is corrupt".into());
    }
    let dependencies: Vec<Dependency> =
        serde_json::from_slice(&fs::read(manifest).map_err(|_| "Cannot read dependency coverage")?)
            .map_err(|_| "Invalid dependency coverage")?;
    for (index, dependency) in dependencies.iter().enumerate() {
        if dependency.link.is_absolute()
            || dependency
                .link
                .components()
                .any(|part| !matches!(part, std::path::Component::Normal(_)))
        {
            return Err("Invalid backed-up dependency path".into());
        }
        let copy = files::checked(
            snapshot,
            &snapshot.join("dependencies").join(index.to_string()),
            false,
        )?;
        verify_copy(&copy, dependency)?;
    }
    Ok(dependencies)
}
pub fn rebase(snapshot: &Path, destination: &Path, dependencies: &[Dependency]) -> Result<()> {
    rebase_subtree(snapshot, destination, dependencies, Path::new(""))
}
pub fn rebase_subtree(
    snapshot: &Path,
    destination: &Path,
    dependencies: &[Dependency],
    prefix: &Path,
) -> Result<()> {
    // Materialize the frozen bytes at each old link, making recovery independent
    // of live generations, original profiles and absolute developer paths.
    for (index, dependency) in dependencies.iter().enumerate() {
        let Ok(relative) = dependency.link.strip_prefix(prefix) else {
            continue;
        };
        let link = destination.join(relative);
        let parent = link.parent().ok_or("Dependency has no parent")?;
        if parent == destination {
            files::root(parent)?;
        } else {
            files::checked(destination, parent, false)?;
        }
        let metadata =
            fs::symlink_metadata(&link).map_err(|_| "Missing restore dependency link")?;
        if !files::is_link(&metadata) {
            return Err("Restore dependency is no longer a link".into());
        }
        #[cfg(windows)]
        if junction::exists(&link).unwrap_or(false) {
            junction::delete(&link).map_err(|_| "Cannot rebase restored junction")?;
        } else {
            fs::remove_file(&link).map_err(|_| "Cannot rebase restored link")?;
        }
        #[cfg(unix)]
        fs::remove_file(&link).map_err(|_| "Cannot rebase restored link")?;
        files::copy_tree(
            &snapshot.join("dependencies").join(index.to_string()),
            &link,
        )?;
        verify_copy(&link, dependency)?;
    }
    Ok(())
}
