//! Versioned extension payloads with per-profile catalogs. Originals remain available.
use crate::managed_files as files;
use crate::profiles::{self, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;
use uuid::Uuid;

pub const CODEX_VERSION: &str = "26.1002.51308";
const SCHEMA: u32 = 1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub version: String,
    pub directory: String,
    pub digest: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Generation {
    pub schema: u32,
    pub id: String,
    pub platform: String,
    pub architecture: String,
    pub packages: Vec<Package>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GenerationCleanup {
    pub digest: String,
    pub generations: Vec<String>,
    pub bytes: u64,
}

pub struct ExtensionStore {
    data_root: PathBuf,
    root: PathBuf,
}

fn component(value: &str) -> Result<()> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_' | b'.'))
        || matches!(value, "." | "..")
    {
        return Err("Invalid extension storage identifier".into());
    }
    Ok(())
}

impl ExtensionStore {
    pub fn open(data_root: &Path) -> Result<Self> {
        files::root(data_root)?;
        let root = data_root.join("shared-vscode");
        for path in [
            &root,
            &root.join("generations"),
            &root.join("staging"),
            &root.join("maintenance-user-data"),
        ] {
            files::directory(data_root, path)?;
        }
        Ok(Self {
            data_root: data_root.into(),
            root,
        })
    }

    fn lock(&self) -> Result<crate::file_security::ExclusiveLock> {
        let path = files::checked(&self.data_root, &self.root.join("operation.lock"), true)?;
        let file = crate::file_security::lock_file(&path)?;
        crate::file_security::exclusive_lock(
            file,
            "Editor extension maintenance is already in progress",
        )
    }

    pub fn generation_path(&self, id: &str) -> Result<PathBuf> {
        if !Uuid::parse_str(id).is_ok_and(|uuid| uuid.to_string() == id) {
            return Err("Invalid shared generation identifier".into());
        }
        files::checked(
            &self.data_root,
            &self.root.join("generations").join(id),
            false,
        )
    }

    pub fn active(&self) -> Result<Option<Generation>> {
        let path = files::checked(
            &self.data_root,
            &self.root.join("active-generation.json"),
            true,
        )?;
        if !path.exists() {
            return Ok(None);
        }
        let id: Value = serde_json::from_slice(
            &fs::read(files::regular(&self.data_root, &path)?)
                .map_err(|_| "Could not read active editor generation")?,
        )
        .map_err(|_| "Active editor generation is invalid")?;
        let directory =
            self.generation_path(id["id"].as_str().ok_or("Missing active generation")?)?;
        let generation: Generation = serde_json::from_slice(
            &fs::read(files::regular(
                &self.data_root,
                &directory.join("manifest.json"),
            )?)
            .map_err(|_| "Could not read shared extension manifest")?,
        )
        .map_err(|_| "Shared extension manifest is invalid")?;
        if generation.id != id["id"].as_str().unwrap() {
            return Err("Shared extension generation mismatch".into());
        }
        self.verify(&generation)?;
        Ok(Some(generation))
    }

    pub fn verify(&self, generation: &Generation) -> Result<()> {
        if generation.schema != SCHEMA
            || generation.platform != std::env::consts::OS
            || generation.architecture != std::env::consts::ARCH
        {
            return Err("Shared extensions do not match this platform or schema".into());
        }
        let directory = self.generation_path(&generation.id)?;
        let mut ids = std::collections::HashSet::new();
        for package in &generation.packages {
            component(&package.directory)?;
            if !ids.insert(&package.id) {
                return Err("Shared extension manifest contains duplicate packages".into());
            }
            let path = files::checked(
                &self.data_root,
                &directory.join("packages").join(&package.directory),
                false,
            )?;
            let manifest = package_manifest(&path)?;
            if package_id(&manifest)? != package.id
                || manifest["version"].as_str() != Some(&package.version)
                || package_digest(&path)? != package.digest
            {
                return Err("Shared extension integrity check failed; use the legacy layout or repair the generation".into());
            }
        }
        if !ids.contains(&"openai.chatgpt".to_owned())
            || !ids.contains(&"russeldanielpaul.multi-codex".to_owned())
        {
            return Err("Shared Codex or its managed bridge is missing".into());
        }
        Ok(())
    }

    /// Publish only a complete package set. Caller checks consumers under its launch guard.
    /// Package installation/staging never edits the current generation or a legacy profile.
    pub fn publish(
        &self,
        sources: &[PathBuf],
        consumers_closed: impl Fn() -> Result<()>,
    ) -> Result<Generation> {
        let _lock = self.lock()?;
        self.publish_locked(sources, consumers_closed)
    }

    fn publish_locked(
        &self,
        sources: &[PathBuf],
        consumers_closed: impl Fn() -> Result<()>,
    ) -> Result<Generation> {
        let mut packages = Vec::new();
        let id = Uuid::new_v4().to_string();
        let stage = self.root.join("staging").join(&id);
        files::directory(&self.data_root, &stage.join("packages"))?;
        for source in sources {
            let manifest = package_manifest(source)?;
            let package_id = package_id(&manifest)?;
            if !matches!(
                package_id.as_str(),
                "openai.chatgpt" | "openai.codex-audio" | "russeldanielpaul.multi-codex"
            ) {
                return Err("Only the managed Codex package set can be published here".into());
            }
            let version = manifest["version"]
                .as_str()
                .ok_or("Extension has no version")?
                .to_owned();
            component(&version)?;
            let name = format!("{package_id}-{version}");
            component(&name)?;
            let digest = package_digest(source)?;
            let destination = stage.join("packages").join(&name);
            files::copy_tree(source, &destination)?;
            if package_digest(&destination)? != digest || package_digest(source)? != digest {
                return Err("Extension package changed during staging; previous generation has been retained".into());
            }
            make_read_only(&destination)?;
            packages.push(Package {
                id: package_id,
                version,
                directory: name,
                digest,
            });
        }
        packages.sort_by(|a, b| a.id.cmp(&b.id));
        let generation = Generation {
            schema: SCHEMA,
            id,
            platform: std::env::consts::OS.into(),
            architecture: std::env::consts::ARCH.into(),
            packages,
        };
        profiles::write_private_file(
            &stage.join("manifest.json"),
            &serde_json::to_vec_pretty(&generation)
                .map_err(|_| "Could not encode shared extension manifest")?,
        )?;
        consumers_closed()?;
        let published = self.root.join("generations").join(&generation.id);
        files::checked(&self.data_root, &published, true)?;
        fs::rename(&stage, &published).map_err(|_| "Could not publish staged editor packages")?;
        crate::file_security::sync_directory(&self.root.join("generations"))?;
        self.verify(&generation)?;
        consumers_closed()?;
        let active = self.root.join("active-generation.json");
        if active.exists() {
            profiles::write_private_file(
                &self.root.join("previous-generation.json"),
                &fs::read(files::regular(&self.data_root, &active)?)
                    .map_err(|_| "Cannot retain previous generation selection")?,
            )?;
        }
        profiles::write_private_file(
            &self.root.join("active-generation.json"),
            &serde_json::to_vec(&json!({"id": generation.id}))
                .map_err(|_| "Could not encode active generation")?,
        )?;
        Ok(generation)
    }

    fn cleanup_inventory(&self) -> Result<GenerationCleanup> {
        let mut retained = std::collections::HashSet::new();
        for pointer in ["active-generation.json", "previous-generation.json"] {
            let path = self.root.join(pointer);
            if path.exists() {
                let value: Value = serde_json::from_slice(
                    &fs::read(files::regular(&self.data_root, &path)?)
                        .map_err(|_| "Cannot read generation selection")?,
                )
                .map_err(|_| "Invalid generation selection")?;
                let id = value["id"].as_str().ok_or("Missing generation selection")?;
                self.generation_path(id)?;
                retained.insert(id.to_owned());
            }
        }
        // Every retained catalog is a consumer, including older catalogs needed
        // for recovery. This collector never discards account metadata or packages.
        let profiles = self.data_root.join("profiles");
        for profile in fs::read_dir(&profiles).map_err(|_| "Cannot inspect generation consumers")? {
            let path = profile
                .map_err(|_| "Cannot inspect generation consumer")?
                .path();
            files::checked(&self.data_root, &path, false)?;
            let catalogs = path.join("extension-catalogs");
            if !catalogs.exists() {
                continue;
            }
            files::checked(&self.data_root, &catalogs, false)?;
            for entry in fs::read_dir(&catalogs).map_err(|_| "Cannot inspect retained catalogs")? {
                let catalog = entry.map_err(|_| "Cannot inspect retained catalog")?.path();
                let path = files::regular(&self.data_root, &catalog.join("generation.json"))?;
                let generation: Generation = serde_json::from_slice(
                    &fs::read(path).map_err(|_| "Cannot read retained catalog")?,
                )
                .map_err(|_| "Invalid retained catalog blocks generation cleanup")?;
                self.generation_path(&generation.id)?;
                retained.insert(generation.id);
            }
        }
        let mut generations = Vec::new();
        let mut rows = Vec::new();
        let mut bytes = 0u64;
        for entry in fs::read_dir(self.root.join("generations"))
            .map_err(|_| "Cannot inventory shared generations")?
        {
            let path = entry
                .map_err(|_| "Cannot inspect shared generation")?
                .path();
            let id = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid generation name")?;
            self.generation_path(id)?;
            if retained.contains(id) {
                continue;
            }
            files::checked(&self.data_root, &path, false)?;
            let manifest = files::tree_manifest(&path)?;
            if manifest.iter().any(|(_, kind)| kind.starts_with("link:")) {
                return Err("Linked generation bytes cannot be collected".into());
            }
            for (relative, kind) in &manifest {
                if kind.starts_with("file:") {
                    bytes = bytes.saturating_add(
                        fs::metadata(path.join(relative))
                            .map_err(|_| "Cannot measure unused generation")?
                            .len(),
                    );
                }
            }
            rows.push((id.to_owned(), files::manifest_digest(&manifest)?));
            generations.push(id.to_owned());
        }
        generations.sort();
        rows.sort();
        Ok(GenerationCleanup {
            digest: files::manifest_digest(
                &rows
                    .iter()
                    .map(|(id, digest)| (PathBuf::from(id), digest.clone()))
                    .collect::<Vec<_>>(),
            )?,
            generations,
            bytes,
        })
    }
    pub fn preview_cleanup(
        &self,
        consumers_closed: impl Fn() -> Result<()>,
    ) -> Result<GenerationCleanup> {
        let _lock = self.lock()?;
        consumers_closed()?;
        self.cleanup_inventory()
    }
    pub fn clear_unused(
        &self,
        preview: &GenerationCleanup,
        consumers_closed: impl Fn() -> Result<()>,
    ) -> Result<u64> {
        let _lock = self.lock()?;
        consumers_closed()?;
        let current = self.cleanup_inventory()?;
        if current.digest != preview.digest
            || current.generations != preview.generations
            || current.bytes != preview.bytes
        {
            return Err("Shared generation cleanup changed. Check its preview again".into());
        }
        fn writable(path: &Path) -> Result<()> {
            let metadata =
                fs::symlink_metadata(path).map_err(|_| "Cannot inspect unused generation")?;
            if files::is_link(&metadata) {
                return Err("Linked generation cleanup is refused".into());
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(
                    path,
                    fs::Permissions::from_mode(if metadata.is_dir() { 0o700 } else { 0o600 }),
                )
                .map_err(|_| "Cannot prepare unused generation cleanup")?;
            }
            #[cfg(windows)]
            {
                let mut permissions = metadata.permissions();
                permissions.set_readonly(false);
                fs::set_permissions(path, permissions)
                    .map_err(|_| "Cannot prepare unused generation cleanup")?;
            }
            if metadata.is_dir() {
                for entry in fs::read_dir(path).map_err(|_| "Cannot inspect unused generation")? {
                    writable(&entry.map_err(|_| "Cannot inspect unused package")?.path())?;
                }
            }
            Ok(())
        }
        for id in &current.generations {
            consumers_closed()?;
            let path = self.generation_path(id)?;
            writable(&path)?;
            fs::remove_dir_all(path).map_err(|_| "Cannot remove unused shared generation")?;
        }
        crate::file_security::sync_directory(&self.root.join("generations"))?;
        Ok(current.bytes)
    }

    /// Uses the existing editor in an isolated installer environment; no editor runtime download.
    pub fn initialize(
        &self,
        code: &Path,
        bridge_package: &Path,
        consumers_closed: impl Fn() -> Result<()>,
    ) -> Result<Generation> {
        let _lock = self.lock()?;
        if let Some(active) = self.active()? {
            // A new app ships a new matching bridge/helper, even when the pinned
            // Codex engine is unchanged. Reuse the verified native package bytes.
            validate_bridge_helper(bridge_package)?;
            let current = active
                .packages
                .iter()
                .find(|package| package.id == "russeldanielpaul.multi-codex")
                .ok_or("Managed bridge package is missing")?;
            if package_digest(bridge_package)? == current.digest {
                return Ok(active);
            }
            let packages = self.generation_path(&active.id)?.join("packages");
            let mut sources: Vec<_> = active
                .packages
                .iter()
                .filter(|package| package.id != "russeldanielpaul.multi-codex")
                .map(|package| packages.join(&package.directory))
                .collect();
            sources.push(bridge_package.into());
            return self.publish_locked(&sources, consumers_closed);
        }
        let temporary = tempfile::Builder::new()
            .prefix("install-")
            .tempdir_in(self.root.join("staging"))
            .map_err(|_| "Cannot allocate private extension installer")?;
        let install = temporary.path().to_path_buf();
        profiles::set_owner_only_dir(&install)?;
        files::directory(&self.data_root, &install.join("extensions"))?;
        files::directory(&self.data_root, &install.join("codex-home"))?;
        profiles::write_codex_config(&install.join("codex-home"))?;
        let mut command = Command::new(code);
        #[cfg(windows)]
        {
            if code.extension().is_none_or(|extension| {
                !extension
                    .to_str()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
            }) {
                return Err(
                    "Choose VS Code's native executable for shared extension maintenance".into(),
                );
            }
            let cli = crate::managed_launch::editor_cli_script(code)?;
            command.arg(cli).env("ELECTRON_RUN_AS_NODE", "1");
        }
        for (key, _) in std::env::vars_os() {
            let name = key.to_string_lossy().to_ascii_uppercase();
            if name.starts_with("CODEX_")
                || name.starts_with("OPENAI_")
                || name.starts_with("VSCODE_")
            {
                command.env_remove(key);
            }
        }
        command
            .env_remove("NODE_OPTIONS")
            .env("CODEX_HOME", install.join("codex-home"))
            .env("CODEX_SQLITE_HOME", install.join("codex-home"))
            .args(["--user-data-dir"])
            .arg(install.join("user-data"))
            .arg("--extensions-dir")
            .arg(install.join("extensions"))
            .arg("--install-extension")
            .arg(format!("openai.chatgpt@{CODEX_VERSION}"))
            .arg("--force");
        #[cfg(not(windows))]
        command.env_remove("ELECTRON_RUN_AS_NODE");
        let result = crate::process::output(&mut command, Duration::from_secs(300))?;
        if !result.status.success() {
            return Err("Could not install the pinned Codex extension. Existing profiles and packages are unchanged".into());
        }
        let bridge = install.join("bridge");
        files::copy_tree(bridge_package, &bridge)?;
        validate_bridge_helper(&bridge)?;
        let mut sources = Vec::new();
        for entry in fs::read_dir(install.join("extensions"))
            .map_err(|_| "Could not inspect installed Codex packages")?
        {
            let path = entry
                .map_err(|_| "Could not inspect installed Codex packages")?
                .path();
            if path.is_dir() {
                sources.push(path);
            }
        }
        sources.push(bridge);
        let codex = sources
            .iter()
            .find(|path| {
                package_manifest(path)
                    .and_then(|m| package_id(&m))
                    .is_ok_and(|id| id == "openai.chatgpt")
            })
            .ok_or("Pinned Codex package missing after installation")?;
        if package_manifest(codex)?["version"].as_str() != Some(CODEX_VERSION) {
            return Err("Editor installed an unexpected Codex version".into());
        }
        self.publish_locked(&sources, consumers_closed)
    }

    /// Build a new private catalog; native deletion of one entry cannot remove peer metadata.
    pub fn stage_catalog(&self, generation: &Generation, profile_id: &str) -> Result<PathBuf> {
        self.stage_catalog_preserving(generation, profile_id, None)
    }

    pub fn stage_catalog_preserving(
        &self,
        generation: &Generation,
        profile_id: &str,
        legacy: Option<&Path>,
    ) -> Result<PathBuf> {
        if !Uuid::parse_str(profile_id).is_ok_and(|uuid| uuid.to_string() == profile_id) {
            return Err("Invalid profile identifier".into());
        }
        let _lock = self.lock()?;
        self.verify(generation)?;
        let catalogs = self
            .data_root
            .join("profiles")
            .join(profile_id)
            .join("extension-catalogs");
        files::directory(&self.data_root, &catalogs)?;
        let catalog = catalogs.join(Uuid::new_v4().to_string());
        files::directory(&self.data_root, &catalog)?;
        let mut registry = Vec::new();
        for package in &generation.packages {
            let target = self
                .generation_path(&generation.id)?
                .join("packages")
                .join(&package.directory);
            files::package_link(&target, &catalog.join(&package.directory))?;
            let link = catalog.join(&package.directory);
            registry.push(json!({"identifier":{"id":package.id},"version":package.version,
                "location":{"$mid":1,"scheme":"file","path":uri_path(&link)?},"relativeLocation":package.directory}));
        }
        if let Some(legacy) = legacy {
            self.preserve_copilot(legacy, &catalog, &mut registry)?;
        }
        profiles::write_private_file(
            &catalog.join("extensions.json"),
            &serde_json::to_vec(&registry)
                .map_err(|_| "Could not encode private extension registry")?,
        )?;
        profiles::write_private_file(
            &catalog.join("generation.json"),
            &serde_json::to_vec(&generation).map_err(|_| "Could not record catalog generation")?,
        )?;
        Ok(catalog)
    }

    pub fn verify_catalog(&self, generation: &Generation, catalog: &Path) -> Result<()> {
        files::checked(&self.data_root, catalog, false)?;
        let recorded: Value = serde_json::from_slice(
            &fs::read(files::regular(
                &self.data_root,
                &catalog.join("generation.json"),
            )?)
            .map_err(|_| "Could not inspect private catalog generation")?,
        )
        .map_err(|_| "Private catalog generation is invalid")?;
        if recorded
            != serde_json::to_value(generation)
                .map_err(|_| "Could not encode catalog generation")?
        {
            return Err(
                "Private catalog selects a different shared generation; recover the profile layout"
                    .into(),
            );
        }
        let registry: Vec<Value> = serde_json::from_slice(
            &fs::read(files::regular(
                &self.data_root,
                &catalog.join("extensions.json"),
            )?)
            .map_err(|_| "Could not inspect private extension registry")?,
        )
        .map_err(|_| "Private extension registry is invalid; recover the profile layout")?;
        for package in &generation.packages {
            let entries: Vec<_> = registry
                .iter()
                .filter(|entry| {
                    entry["identifier"]["id"]
                        .as_str()
                        .is_some_and(|value| value.eq_ignore_ascii_case(&package.id))
                })
                .collect();
            let [entry] = entries.as_slice() else {
                return Err("A required shared extension was removed or its registry changed. Close this profile and return to its original layout".into());
            };
            let link = catalog.join(&package.directory);
            let target = self
                .generation_path(&generation.id)?
                .join("packages")
                .join(&package.directory);
            let metadata = fs::symlink_metadata(&link).map_err(|_| {
                "A required shared extension link is missing; recover the profile layout"
            })?;
            if !files::is_link(&metadata)
                || fs::canonicalize(files::link_target(&link)?)
                    .map_err(|_| "Shared extension target is unavailable")?
                    != fs::canonicalize(&target)
                        .map_err(|_| "Shared generation target is unavailable")?
                || entry["version"] != package.version
                || entry["relativeLocation"] != package.directory
                || entry["location"]["scheme"] != "file"
                || entry["location"]["path"] != uri_path(&link)?
            {
                return Err("Private extension registry or link no longer matches its verified generation; recover the profile layout".into());
            }
        }
        for entry in &registry {
            let name = entry["relativeLocation"]
                .as_str()
                .ok_or("Invalid private extension location")?;
            component(name)?;
            if generation
                .packages
                .iter()
                .any(|package| package.directory == name)
            {
                continue;
            }
            let directory = files::checked(&self.data_root, &catalog.join(name), false)?;
            files::root(&directory)?;
            files::regular(&directory, &directory.join("package.json"))?;
            if entry["location"]["scheme"] != "file"
                || entry["location"]["path"] != uri_path(&directory)?
            {
                return Err("Private extension registry points outside its profile; recover the profile layout".into());
            }
        }
        Ok(())
    }

    fn preserve_copilot(
        &self,
        legacy: &Path,
        catalog: &Path,
        registry: &mut Vec<Value>,
    ) -> Result<()> {
        files::checked(&self.data_root, legacy, false)?;
        let mut installed = std::collections::BTreeMap::new();
        for entry in
            fs::read_dir(legacy).map_err(|_| "Could not inspect existing extension packages")?
        {
            let path = entry
                .map_err(|_| "Could not inspect existing extension package")?
                .path();
            let metadata =
                fs::symlink_metadata(&path).map_err(|_| "Could not inspect extension entry")?;
            if !metadata.is_dir() {
                continue;
            }
            if files::is_link(&metadata) {
                return Err(
                    "Linked legacy extension packages require explicit migration coverage".into(),
                );
            }
            let manifest = package_manifest(&path)?;
            let id = package_id(&manifest)?;
            installed
                .entry(id)
                .or_insert_with(Vec::new)
                .push((path, manifest));
        }
        let original_registry = legacy.join("extensions.json");
        let entries: Vec<Value> = if original_registry.exists() {
            serde_json::from_slice(
                &fs::read(files::regular(&self.data_root, &original_registry)?)
                    .map_err(|_| "Could not inspect legacy extension registry")?,
            )
            .map_err(|_| "Legacy extension registry is invalid; migration has been deferred")?
        } else {
            Vec::new()
        };
        let mut pending = vec![
            "github.copilot".to_owned(),
            "github.copilot-chat".to_owned(),
        ];
        let mut preserved = std::collections::HashSet::new();
        while let Some(id) = pending.pop() {
            if !preserved.insert(id.clone()) {
                continue;
            }
            if registry.iter().any(|entry| {
                entry["identifier"]["id"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(&id))
            }) {
                continue;
            }
            let Some(candidates) = installed.get(&id) else {
                if matches!(id.as_str(), "github.copilot" | "github.copilot-chat") {
                    continue;
                }
                return Err(
                    "A required Copilot dependency is missing; keep the legacy layout".into(),
                );
            };
            let entry = entries.iter().find(|entry| {
                entry["identifier"]["id"]
                    .as_str()
                    .is_some_and(|value| value.eq_ignore_ascii_case(&id))
            });
            let selected = if let Some(entry) = entry {
                candidates
                    .iter()
                    .find(|(path, manifest)| {
                        entry["version"] == manifest["version"]
                            && entry["relativeLocation"].as_str().is_none_or(|name| {
                                path.file_name().and_then(|name| name.to_str()) == Some(name)
                            })
                    })
                    .ok_or("Copilot registry and package version do not agree")?
            } else if candidates.len() == 1 {
                &candidates[0]
            } else {
                return Err(
                    "Copilot package selection is ambiguous; keep the legacy layout".into(),
                );
            };
            let (source, manifest) = selected;
            if let Some(dependencies) = manifest.get("extensionDependencies") {
                for dependency in dependencies
                    .as_array()
                    .ok_or("Copilot dependency manifest is invalid")?
                {
                    let dependency = dependency
                        .as_str()
                        .ok_or("Copilot dependency manifest is invalid")?;
                    component(dependency)?;
                    pending.push(dependency.to_ascii_lowercase());
                }
            }
            let name = source
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("Invalid Copilot package path")?;
            component(name)?;
            let target = catalog.join(name);
            let digest = package_digest(source)?;
            files::copy_tree(source, &target)?;
            if package_digest(&target)? != digest || package_digest(source)? != digest {
                return Err(
                    "Copilot changed during migration; legacy packages remain intact".into(),
                );
            }
            let mut record = entry
                .cloned()
                .unwrap_or_else(|| json!({"identifier":{"id":id},"version":manifest["version"]}));
            record["location"] = json!({"$mid":1,"scheme":"file","path":uri_path(&target)?});
            record["relativeLocation"] = Value::String(name.into());
            registry.push(record);
        }
        Ok(())
    }
}

fn uri_path(path: &Path) -> Result<String> {
    let path = path
        .to_str()
        .ok_or("Editor path is not valid Unicode")?
        .replace('\\', "/");
    #[cfg(windows)]
    let path = if path.as_bytes().get(1) == Some(&b':') {
        format!("/{path}")
    } else {
        path
    };
    Ok(path)
}

fn validate_bridge_helper(bridge: &Path) -> Result<()> {
    let helper_name = if cfg!(windows) {
        "multi-codex-account-helper.exe"
    } else {
        "multi-codex-account-helper"
    };
    let helper_path = files::regular(bridge, &bridge.join("bin").join(helper_name))?;
    let helper: Value = serde_json::from_slice(
        &fs::read(files::regular(bridge, &bridge.join("desktop-helper.json"))?)
            .map_err(|_| "Matching native helper metadata is unavailable")?,
    )
    .map_err(|_| "Matching native helper metadata is invalid")?;
    if helper["schema"] != 1
        || helper["os"] != std::env::consts::OS
        || helper["architecture"] != std::env::consts::ARCH
        || helper["version"] != env!("CARGO_PKG_VERSION")
        || helper["digest"].as_str() != Some(&files::digest(&helper_path)?)
        || !profiles::is_executable_file(&helper_path)
    {
        return Err("Managed bridge helper does not match this app version and platform".into());
    }
    Ok(())
}

fn package_manifest(path: &Path) -> Result<Value> {
    files::root(path)?;
    let manifest: Value = serde_json::from_slice(
        &fs::read(files::regular(path, &path.join("package.json"))?)
            .map_err(|_| "Could not read extension package")?,
    )
    .map_err(|_| "Extension package manifest is invalid")?;
    if let Some(main) = manifest["main"].as_str() {
        files::regular(path, &path.join(main.trim_start_matches("./")))?;
    }
    Ok(manifest)
}

fn package_id(manifest: &Value) -> Result<String> {
    let publisher = manifest["publisher"]
        .as_str()
        .ok_or("Extension publisher is missing")?;
    let name = manifest["name"]
        .as_str()
        .ok_or("Extension name is missing")?;
    component(publisher)?;
    component(name)?;
    Ok(format!("{publisher}.{name}").to_ascii_lowercase())
}

fn package_digest(path: &Path) -> Result<String> {
    let rows = files::tree_manifest(path)?;
    if rows.iter().any(|(_, value)| value.starts_with("link:")) {
        return Err("Shared extension payloads cannot contain linked files".into());
    }
    // Publication intentionally removes write permissions. Retain execution
    // identity in the package digest while excluding that expected change.
    let rows: Vec<_> = rows
        .into_iter()
        .map(|(path, value)| {
            let value = if let Some(attributes) = value.strip_prefix("file:") {
                if let Some((mode, digest)) = attributes.split_once(':') {
                    #[cfg(unix)]
                    let execution = u32::from_str_radix(mode, 8).unwrap_or_default() & 0o100;
                    #[cfg(windows)]
                    let execution = {
                        let _ = mode;
                        0
                    };
                    format!("file:{execution}:{digest}")
                } else {
                    value
                }
            } else {
                value
            };
            (path, value)
        })
        .collect();
    files::manifest_digest(&rows)
}

fn make_read_only(path: &Path) -> Result<()> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| "Could not inspect staged package permissions")?;
    if files::is_link(&metadata) {
        return Err("Shared package permissions cannot follow links".into());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(|_| "Could not protect staged package")? {
            make_read_only(
                &entry
                    .map_err(|_| "Could not protect staged package")?
                    .path(),
            )?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if metadata.is_dir() || metadata.permissions().mode() & 0o100 != 0 {
            0o500
        } else {
            0o400
        };
        fs::set_permissions(path, fs::Permissions::from_mode(mode))
            .map_err(|_| "Could not protect shared package")?;
    }
    #[cfg(windows)]
    {
        let mut permissions = metadata.permissions();
        permissions.set_readonly(true);
        fs::set_permissions(path, permissions).map_err(|_| "Could not protect shared package")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (tempfile::TempDir, ExtensionStore, Vec<PathBuf>) {
        let temp = tempfile::tempdir().unwrap();
        let app = temp.path().join("app");
        fs::create_dir(&app).unwrap();
        let mut sources = Vec::new();
        for (publisher, name) in [("openai", "chatgpt"), ("russeldanielpaul", "multi-codex")] {
            let package = temp.path().join(name);
            fs::create_dir(&package).unwrap();
            fs::write(package.join("package.json"), serde_json::to_vec(&json!({"publisher":publisher,"name":name,"version":"1.0.0","main":"./extension.js"})).unwrap()).unwrap();
            fs::write(package.join("extension.js"), "fixture package").unwrap();
            sources.push(package);
        }
        (temp, ExtensionStore::open(&app).unwrap(), sources)
    }
    // Protected payloads need writable directory entries only during fixture cleanup.
    fn unprotect(path: &Path) {
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
                unprotect(&entry.unwrap().path());
            }
        }
    }
    #[test]
    fn private_catalog_removal_preserves_shared_payload_and_peer() {
        let (_temp, store, sources) = fixture();
        let generation = store.publish(&sources, || Ok(())).unwrap();
        let a = store
            .stage_catalog(&generation, &Uuid::new_v4().to_string())
            .unwrap();
        let b = store
            .stage_catalog(&generation, &Uuid::new_v4().to_string())
            .unwrap();
        let name = &generation.packages[0].directory;
        store.verify_catalog(&generation, &a).unwrap();
        store.verify_catalog(&generation, &b).unwrap();
        #[cfg(unix)]
        fs::remove_file(b.join(name)).unwrap();
        #[cfg(windows)]
        junction::delete(b.join(name)).unwrap();
        assert!(a.join(name).join("package.json").exists());
        assert!(store.verify_catalog(&generation, &b).is_err());
        store.verify_catalog(&generation, &a).unwrap();
        store.verify(&generation).unwrap();
        unprotect(&store.root);
    }
    #[test]
    fn failed_maintenance_never_replaces_active_generation() {
        let (_temp, store, sources) = fixture();
        let first = store.publish(&sources, || Ok(())).unwrap();
        assert!(store
            .publish(&sources, || Err("Consumer is running".into()))
            .is_err());
        assert_eq!(store.active().unwrap().unwrap().id, first.id);
        unprotect(&store.root);
    }
    #[test]
    fn copilot_and_required_dependencies_remain_private_with_registry_metadata() {
        let (_temp, store, sources) = fixture();
        let generation = store.publish(&sources, || Ok(())).unwrap();
        let id = Uuid::new_v4().to_string();
        let legacy = store
            .data_root
            .join("profiles")
            .join(&id)
            .join("vscode-extensions");
        files::directory(&store.data_root, &legacy).unwrap();
        for (publisher, name, dependencies) in [
            ("github", "copilot", vec!["Fixture.Dependency"]),
            ("fixture", "dependency", vec![]),
            ("fixture", "extra", vec![]),
        ] {
            let package = legacy.join(format!("{publisher}.{name}-1.0.0"));
            fs::create_dir(&package).unwrap();
            fs::write(package.join("package.json"), serde_json::to_vec(&json!({"publisher":publisher,"name":name,"version":"1.0.0","main":"main.js","extensionDependencies":dependencies})).unwrap()).unwrap();
            fs::write(package.join("main.js"), "original").unwrap();
        }
        fs::write(legacy.join("extensions.json"), serde_json::to_vec(&json!([{"identifier":{"id":"GitHub.Copilot","uuid":"existing-id"},"version":"1.0.0","relativeLocation":"github.copilot-1.0.0","metadata":{"installedTimestamp":42}}])).unwrap()).unwrap();
        let before = files::tree_manifest(&legacy).unwrap();
        let catalog = store
            .stage_catalog_preserving(&generation, &id, Some(&legacy))
            .unwrap();
        assert!(catalog.join("github.copilot-1.0.0/main.js").exists());
        assert!(catalog.join("fixture.dependency-1.0.0/main.js").exists());
        assert!(!catalog.join("fixture.extra-1.0.0").exists());
        assert!(!files::is_link(
            &fs::symlink_metadata(catalog.join("github.copilot-1.0.0")).unwrap()
        ));
        fs::write(catalog.join("github.copilot-1.0.0/main.js"), "private edit").unwrap();
        assert_eq!(before, files::tree_manifest(&legacy).unwrap());
        let registry: Value =
            serde_json::from_slice(&fs::read(catalog.join("extensions.json")).unwrap()).unwrap();
        let copilot = registry
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["identifier"]["id"] == "GitHub.Copilot")
            .unwrap();
        assert_eq!(copilot["metadata"]["installedTimestamp"], 42);
        assert_eq!(copilot["identifier"]["uuid"], "existing-id");
        unprotect(&store.root);
    }
    #[test]
    fn corrupt_payload_and_traversal_metadata_are_rejected() {
        let (_temp, store, sources) = fixture();
        let generation = store.publish(&sources, || Ok(())).unwrap();
        let path = store
            .generation_path(&generation.id)
            .unwrap()
            .join("packages")
            .join(&generation.packages[0].directory);
        unprotect(&path);
        fs::write(path.join("extension.js"), "corrupted").unwrap();
        assert!(store.verify(&generation).is_err());
        let mut invalid = generation;
        invalid.packages[0].directory = "../outside".into();
        assert!(store.verify(&invalid).is_err());
        unprotect(&store.root);
    }
    #[cfg(unix)]
    #[test]
    fn linked_payload_is_never_published() {
        let (_temp, store, sources) = fixture();
        std::os::unix::fs::symlink("extension.js", sources[0].join("linked-code")).unwrap();
        assert!(store.publish(&sources, || Ok(())).is_err());
        assert!(store.active().unwrap().is_none());
        unprotect(&store.root);
    }
    #[test]
    fn generation_cleanup_retains_current_previous_and_catalog_consumers() {
        let (_temp, store, sources) = fixture();
        fs::create_dir_all(store.data_root.join("profiles")).unwrap();
        let first = store.publish(&sources, || Ok(())).unwrap();
        let second = store.publish(&sources, || Ok(())).unwrap();
        let third = store.publish(&sources, || Ok(())).unwrap();
        let preview = store.preview_cleanup(|| Ok(())).unwrap();
        assert_eq!(preview.generations, vec![first.id.clone()]);
        assert!(store
            .clear_unused(&preview, || Err("consumer started".into()))
            .is_err());
        let profile = Uuid::new_v4().to_string();
        store.stage_catalog(&first, &profile).unwrap();
        assert!(store.clear_unused(&preview, || Ok(())).is_err());
        assert!(store
            .preview_cleanup(|| Ok(()))
            .unwrap()
            .generations
            .is_empty());
        // A separate abandoned publication can be reclaimed without touching
        // either selected generation or a retained catalog consumer.
        let fourth = store.publish(&sources, || Ok(())).unwrap();
        let unused = store.preview_cleanup(|| Ok(())).unwrap();
        assert_eq!(unused.generations, vec![second.id.clone()]);
        assert!(store.clear_unused(&unused, || Ok(())).unwrap() > 0);
        assert!(!store.root.join("generations").join(&second.id).exists());
        store.verify(&first).unwrap();
        store.verify(&third).unwrap();
        store.verify(&fourth).unwrap();
        unprotect(&store.data_root);
    }
}
