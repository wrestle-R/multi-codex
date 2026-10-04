use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::usage::{read_profile_limits, ProfileLimits};

const MAX_AUTH_BYTES: usize = 1024 * 1024;
const KEYRING_SERVICE: &str = "multi-codex";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileInput {
    pub name: String,
    pub auth_json: String,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMetadata {
    pub id: String,
    pub name: String,
    pub auth_mode: String,
    #[serde(default)]
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub metadata: ProfileMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_tier: Option<String>,
    pub status: RuntimeStatus,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeStatus {
    Idle,
    Running,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRuntime {
    pub id: String,
    pub status: RuntimeStatus,
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingDeviceLogin {
    pub id: String,
    pub name: String,
    pub notes: Option<String>,
    pub codex_home: PathBuf,
    root: PathBuf,
    replace_profile_id: Option<String>,
}

#[derive(Clone, Debug)]
struct ProfilePaths {
    codex_home: PathBuf,
    vscode_home: PathBuf,
    extensions_dir: PathBuf,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStorageUsage {
    pub id: String,
    pub name: String,
    pub bytes: u64,
    pub reclaimable_bytes: u64,
    pub running: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub bytes: u64,
    pub reclaimable_bytes: u64,
    pub other_bytes: u64,
    pub profiles: Vec<ProfileStorageUsage>,
}

const RECLAIMABLE_PATHS: &[&[&str]] = &[
    &["vscode-user-data", "CachedExtensionVSIXs"],
    &["vscode-user-data", "Cache"],
    &["vscode-user-data", "CachedData"],
    &["vscode-user-data", "GPUCache"],
    &["vscode-user-data", "Code Cache"],
    &["vscode-user-data", "Crashpad"],
    &["vscode-user-data", "logs"],
    &["codex-home", "cache"],
    &["codex-home", ".tmp"],
    &["codex-home", "tmp"],
    &["codex-home", "log"],
];

#[derive(Default)]
struct RuntimeState {
    statuses: HashMap<String, RuntimeStatus>,
    errors: HashMap<String, String>,
    monitors: HashSet<String>,
}

pub trait SecretStore: Send + Sync + 'static {
    fn set(&self, id: &str, secret: &str) -> Result<()>;
    fn get(&self, id: &str) -> Result<String>;
    fn delete(&self, id: &str) -> Result<()>;
}

pub struct KeyringSecretStore;

impl SecretStore for KeyringSecretStore {
    fn set(&self, id: &str, secret: &str) -> Result<()> {
        keyring::Entry::new(KEYRING_SERVICE, id)
            .and_then(|entry| entry.set_password(secret))
            .map_err(|_| "The system credential store could not save this credential".to_string())
    }

    fn get(&self, id: &str) -> Result<String> {
        keyring::Entry::new(KEYRING_SERVICE, id)
            .and_then(|entry| entry.get_password())
            .map_err(|_| "The system credential store could not read this credential".to_string())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let entry = keyring::Entry::new(KEYRING_SERVICE, id)
            .map_err(|_| "The system credential store is unavailable".to_string())?;
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => {
                Err("The system credential store could not delete this credential".to_string())
            }
        }
    }
}

pub trait AuthRecognizer: Send + Sync + 'static {
    fn recognize(&self, auth_json: &str) -> Result<()>;
}

pub struct CodexCliRecognizer;

impl AuthRecognizer for CodexCliRecognizer {
    fn recognize(&self, auth_json: &str) -> Result<()> {
        let temp = tempfile::Builder::new()
            .prefix("multi-codex-validation-")
            .tempdir()
            .map_err(|error| format!("Could not create a validation directory: {error}"))?;
        set_owner_only_dir(temp.path())?;
        write_private_file(&temp.path().join("auth.json"), auth_json.as_bytes())?;
        write_codex_config(temp.path())?;

        let codex = resolve_codex_command()?;
        let output = Command::new(codex)
            .args(["login", "status"])
            .env("CODEX_HOME", temp.path())
            .output()
            .map_err(|_| "Codex CLI could not be started to validate this account".to_string())?;
        if output.status.success() {
            Ok(())
        } else {
            Err("Codex did not recognize this auth JSON as a logged-in account".to_string())
        }
    }
}

pub struct ProfileService<S: SecretStore, R: AuthRecognizer> {
    data_root: PathBuf,
    global_codex_home: PathBuf,
    secrets: Arc<S>,
    recognizer: Arc<R>,
    runtime: Arc<Mutex<RuntimeState>>,
}

impl<S: SecretStore, R: AuthRecognizer> ProfileService<S, R> {
    pub fn new(
        data_root: PathBuf,
        global_codex_home: PathBuf,
        _extensions_dir: PathBuf,
        secrets: S,
        recognizer: R,
    ) -> Result<Self> {
        ensure_private_dir(&data_root)?;
        let canonical_data = fs::canonicalize(&data_root)
            .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
        if let Ok(canonical_global) = fs::canonicalize(&global_codex_home) {
            if canonical_data == canonical_global
                || canonical_data.starts_with(&canonical_global)
                || canonical_global.starts_with(&canonical_data)
            {
                return Err("Multi Codex data must be outside the default Codex home".to_string());
            }
        }
        ensure_private_dir(&data_root.join("profiles"))?;
        let service = Self {
            data_root,
            global_codex_home,
            secrets: Arc::new(secrets),
            recognizer: Arc::new(recognizer),
            runtime: Arc::new(Mutex::new(RuntimeState::default())),
        };
        Ok(service)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileView>> {
        let mut profiles = self.load_metadata()?;
        profiles.sort_by_key(|profile| std::cmp::Reverse(profile.updated_at));
        let (statuses, errors) = {
            let runtime = self
                .runtime
                .lock()
                .map_err(|_| "Runtime state is unavailable")?;
            (runtime.statuses.clone(), runtime.errors.clone())
        };
        let mut views = Vec::with_capacity(profiles.len());
        for metadata in profiles {
            let detected = profile_process_running(&self.profile_paths(&metadata.id)?.vscode_home);
            let status = statuses
                .get(&metadata.id)
                .copied()
                .filter(|status| *status != RuntimeStatus::Running || detected)
                .unwrap_or(if detected {
                    RuntimeStatus::Running
                } else {
                    RuntimeStatus::Idle
                });
            let error = errors.get(&metadata.id).cloned();
            // Avoid a system-keyring round trip on every UI poll. The durable profile credential
            // is also the freshest copy because Codex writes token refreshes into this file.
            let account_tier = self
                .read_persisted_profile_credential(&metadata.id)
                .ok()
                .flatten()
                .and_then(|secret| account_tier_from_auth(&secret));
            views.push(ProfileView {
                metadata,
                account_tier,
                status,
                error,
            });
        }
        Ok(views)
    }

    pub fn add_profile(&self, input: SaveProfileInput) -> Result<ProfileView> {
        let name = validate_name(&input.name)?;
        let notes = validate_notes(input.notes)?;
        let auth_mode = validate_auth_structure(&input.auth_json)?;
        self.recognizer.recognize(&input.auth_json)?;
        let mut profiles = self.load_metadata()?;
        ensure_unique_name(&profiles, &name, None)?;
        let now = Utc::now();
        let metadata = ProfileMetadata {
            id: Uuid::new_v4().to_string(),
            name,
            auth_mode,
            notes,
            created_at: now,
            updated_at: now,
        };
        self.secrets.set(&metadata.id, &input.auth_json)?;
        if let Err(error) = self.persist_profile_credential(&metadata.id, &input.auth_json) {
            let _ = self.secrets.delete(&metadata.id);
            return Err(error);
        }
        profiles.push(metadata.clone());
        if let Err(error) = self.save_metadata(&profiles) {
            let _ = self.secrets.delete(&metadata.id);
            if let Ok(paths) = self.profile_paths(&metadata.id) {
                let _ = remove_managed_tree(
                    &self.data_root,
                    paths.codex_home.parent().unwrap_or(&paths.codex_home),
                );
            }
            return Err(error);
        }
        Ok(ProfileView {
            metadata,
            account_tier: account_tier_from_auth(&input.auth_json),
            status: RuntimeStatus::Idle,
            error: None,
        })
    }

    pub fn import_current(&self, name: String, notes: Option<String>) -> Result<ProfileView> {
        let auth_path = self.global_codex_home.join("auth.json");
        let metadata = fs::symlink_metadata(&auth_path)
            .map_err(|_| "The current Codex auth file could not be read".to_string())?;
        if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
            return Err("The current Codex auth path is not a regular file".to_string());
        }
        if metadata.len() as usize > MAX_AUTH_BYTES {
            return Err("The current Codex auth file is too large".to_string());
        }
        let auth_json = fs::read_to_string(&auth_path)
            .map_err(|_| "The current Codex auth file could not be read".to_string())?;
        self.add_profile(SaveProfileInput {
            name,
            auth_json,
            notes,
        })
    }

    pub(crate) fn prepare_device_login(
        &self,
        name: String,
        notes: Option<String>,
    ) -> Result<PendingDeviceLogin> {
        let name = validate_name(&name)?;
        let notes = validate_notes(notes)?;
        ensure_unique_name(&self.load_metadata()?, &name, None)?;
        let id = Uuid::new_v4().to_string();
        let root = self.data_root.join("device-logins").join(&id);
        let codex_home = root.join("codex-home");
        ensure_private_managed_dir(&self.data_root, &codex_home)?;
        write_codex_config(&codex_home)?;
        Ok(PendingDeviceLogin {
            id,
            name,
            notes,
            codex_home,
            root,
            replace_profile_id: None,
        })
    }

    pub(crate) fn prepare_profile_reauthentication(
        &self,
        profile_id: &str,
    ) -> Result<PendingDeviceLogin> {
        validate_id(profile_id)?;
        if self.is_running(profile_id)? {
            return Err("Close this profile's VS Code window before signing in again".to_string());
        }
        let profile = self
            .load_metadata()?
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| "Profile not found".to_string())?;
        let id = Uuid::new_v4().to_string();
        let root = self.data_root.join("device-logins").join(&id);
        let codex_home = root.join("codex-home");
        ensure_private_managed_dir(&self.data_root, &codex_home)?;
        write_codex_config(&codex_home)?;
        Ok(PendingDeviceLogin {
            id,
            name: profile.name,
            notes: profile.notes,
            codex_home,
            root,
            replace_profile_id: Some(profile_id.to_string()),
        })
    }

    pub(crate) fn finish_device_login(&self, pending: PendingDeviceLogin) -> Result<ProfileView> {
        let auth_json = fs::read_to_string(pending.codex_home.join("auth.json"))
            .map_err(|_| "The browser sign-in did not create a credential".to_string())?;
        let result = if let Some(profile_id) = &pending.replace_profile_id {
            self.update_profile(
                profile_id,
                pending.name.clone(),
                Some(auth_json),
                pending.notes.clone(),
            )
        } else {
            self.add_profile(SaveProfileInput {
                name: pending.name.clone(),
                auth_json,
                notes: pending.notes.clone(),
            })
        };
        let cleanup = remove_managed_tree(&self.data_root, &pending.root);
        match (result, cleanup) {
            (Ok(profile), Ok(())) => Ok(profile),
            (Ok(_), Err(error)) => Err(error),
            (Err(error), _) => Err(error),
        }
    }

    pub(crate) fn abandon_device_login(&self, pending: PendingDeviceLogin) -> Result<()> {
        remove_managed_tree(&self.data_root, &pending.root)
    }

    pub fn update_profile(
        &self,
        id: &str,
        name: String,
        auth_json: Option<String>,
        notes: Option<String>,
    ) -> Result<ProfileView> {
        validate_id(id)?;
        if self.is_running(id)? {
            return Err("Close this profile's VS Code window before editing it".to_string());
        }
        let name = validate_name(&name)?;
        let notes = validate_notes(notes)?;
        let mut profiles = self.load_metadata()?;
        ensure_unique_name(&profiles, &name, Some(id))?;
        let index = profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or_else(|| "Profile not found".to_string())?;
        let previous_secret = if auth_json.is_some() {
            Some(self.secrets.get(id)?)
        } else {
            None
        };
        let previous_file_auth = if auth_json.is_some() {
            self.read_persisted_profile_credential(id)?
        } else {
            None
        };
        let auth_mode = if let Some(ref value) = auth_json {
            let mode = validate_auth_structure(value)?;
            self.recognizer.recognize(value)?;
            self.secrets.set(id, value)?;
            if let Err(error) = self.persist_profile_credential(id, value) {
                if let Some(previous) = &previous_secret {
                    let _ = self.secrets.set(id, previous);
                }
                return Err(error);
            }
            mode
        } else {
            profiles[index].auth_mode.clone()
        };
        profiles[index].name = name;
        profiles[index].auth_mode = auth_mode;
        profiles[index].notes = notes;
        profiles[index].updated_at = Utc::now();
        let metadata = profiles[index].clone();
        if let Err(error) = self.save_metadata(&profiles) {
            if let Some(previous) = previous_secret {
                let _ = self.secrets.set(id, &previous);
            }
            match previous_file_auth {
                Some(previous) => {
                    let _ = self.persist_profile_credential(id, &previous);
                }
                None if auth_json.is_some() => {
                    if let Ok(paths) = self.profile_paths(id) {
                        let _ = remove_private_file(&paths.codex_home.join("auth.json"));
                    }
                }
                None => {}
            }
            return Err(error);
        }
        Ok(ProfileView {
            metadata,
            account_tier: self
                .secrets
                .get(id)
                .ok()
                .and_then(|secret| account_tier_from_auth(&secret)),
            status: RuntimeStatus::Idle,
            error: None,
        })
    }

    pub fn delete_profile(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        if self.is_running(id)? {
            return Err("Close this profile's VS Code window before deleting it".to_string());
        }
        let mut profiles = self.load_metadata()?;
        if !profiles.iter().any(|profile| profile.id == id) {
            return Err("Profile not found".to_string());
        }
        let previous_secret = self.secrets.get(id)?;
        self.secrets.delete(id)?;
        profiles.retain(|profile| profile.id != id);
        if let Err(error) = self.save_metadata(&profiles) {
            let _ = self.secrets.set(id, &previous_secret);
            return Err(error);
        }
        let paths = self.profile_paths(id)?;
        remove_managed_tree(
            &self.data_root,
            paths.codex_home.parent().unwrap_or(&paths.codex_home),
        )?;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        runtime.statuses.remove(id);
        runtime.errors.remove(id);
        Ok(())
    }

    pub fn launch_profile(&self, id: &str, workspace: &Path) -> Result<()> {
        let code = resolve_command("code")?;
        self.launch_profile_with_command(id, workspace, &code)
    }

    fn launch_profile_with_command(&self, id: &str, workspace: &Path, code: &Path) -> Result<()> {
        validate_id(id)?;
        if !self.load_metadata()?.iter().any(|profile| profile.id == id) {
            return Err("Profile not found".to_string());
        }
        let workspace = canonical_workspace(workspace)?;
        let paths = self.profile_paths(id)?;
        ensure_private_managed_dir(&self.data_root, &paths.codex_home)?;
        ensure_private_managed_dir(&self.data_root, &paths.vscode_home)?;
        ensure_private_managed_dir(&self.data_root, &paths.extensions_dir)?;
        write_codex_config(&paths.codex_home)?;
        let auth_path = paths.codex_home.join("auth.json");
        self.current_profile_credential(id)?;
        let launch_workspace = create_launch_workspace(
            &self.data_root,
            paths
                .vscode_home
                .parent()
                .ok_or("Profile directory unavailable")?,
            &workspace,
        )?;

        let mut command = build_vscode_command_with(
            code,
            &paths.codex_home,
            &paths.vscode_home,
            &paths.extensions_dir,
            &launch_workspace,
        );
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                if let Some(directory) = launch_workspace.parent() {
                    let _ = remove_managed_tree(&self.data_root, directory);
                }
                let message = format!("Could not launch VS Code: {error}");
                self.set_error(id, message.clone());
                return Err(message);
            }
        };

        {
            let mut runtime = self
                .runtime
                .lock()
                .map_err(|_| "Runtime state is unavailable")?;
            runtime
                .statuses
                .insert(id.to_string(), RuntimeStatus::Running);
            runtime.errors.remove(id);
        }

        // The `code` helper returns after forwarding a window request, so its lifetime does not
        // represent the isolated VS Code window. Reap it separately and monitor the profile's
        // user-data directory instead. The durable auth file stays available for every window.
        std::thread::spawn(move || {
            let mut child = child;
            let _ = child.wait();
        });
        self.start_profile_monitor(id, paths.vscode_home, auth_path)?;
        Ok(())
    }

    pub fn runtime_status(&self, id: &str) -> Result<ProfileRuntime> {
        validate_id(id)?;
        let paths = self.profile_paths(id)?;
        let detected = profile_process_running(&paths.vscode_home);
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        let status = runtime.statuses.get(id).copied().unwrap_or(if detected {
            RuntimeStatus::Running
        } else {
            RuntimeStatus::Idle
        });
        Ok(ProfileRuntime {
            id: id.to_string(),
            status,
            error: runtime.errors.get(id).cloned(),
        })
    }

    pub fn check_profile_limits(&self, id: &str) -> Result<ProfileLimits> {
        validate_id(id)?;
        let profile = self
            .load_metadata()?
            .into_iter()
            .find(|profile| profile.id == id)
            .ok_or_else(|| "Profile not found".to_string())?;
        if !profile.auth_mode.eq_ignore_ascii_case("chatgpt") {
            return Err("Live limits are available only for ChatGPT accounts".to_string());
        }
        self.current_profile_credential(id)?;
        let paths = self.profile_paths(id)?;
        let result = read_profile_limits(&paths.codex_home);

        // Codex can refresh and rotate credentials while serving the limits request. Always copy
        // that durable result back to the keyring, even when the limits request itself failed.
        if let Ok(refreshed) = read_persisted_credential(&paths.codex_home.join("auth.json")) {
            self.secrets.set(id, &refreshed)?;
        }
        result
    }

    pub fn storage_usage(&self) -> Result<StorageUsage> {
        let bytes = directory_size(&self.data_root)?;
        let mut profile_bytes = 0_u64;
        let mut reclaimable_bytes = 0_u64;
        let mut profiles = Vec::new();
        for profile in self.load_metadata()? {
            let root = self.data_root.join("profiles").join(&profile.id);
            let size = if root.exists() {
                directory_size(&root)?
            } else {
                0
            };
            let reclaimable = reclaimable_size(&root)?;
            let running = self.is_running(&profile.id)?;
            profile_bytes = profile_bytes.saturating_add(size);
            reclaimable_bytes = reclaimable_bytes.saturating_add(reclaimable);
            profiles.push(ProfileStorageUsage {
                id: profile.id,
                name: profile.name,
                bytes: size,
                reclaimable_bytes: reclaimable,
                running,
            });
        }
        profiles.sort_by_key(|profile| std::cmp::Reverse(profile.bytes));
        Ok(StorageUsage {
            bytes,
            reclaimable_bytes,
            other_bytes: bytes.saturating_sub(profile_bytes),
            profiles,
        })
    }

    pub fn clear_profile_cache(&self, id: &str) -> Result<u64> {
        validate_id(id)?;
        if !self.load_metadata()?.iter().any(|profile| profile.id == id) {
            return Err("Profile not found".to_string());
        }
        if self.is_running(id)? {
            return Err(
                "Close this profile's VS Code windows before clearing its cache".to_string(),
            );
        }
        let root = self.data_root.join("profiles").join(id);
        let reclaimed = reclaimable_size(&root)?;
        for components in RECLAIMABLE_PATHS {
            let path = components
                .iter()
                .fold(root.clone(), |path, component| path.join(component));
            remove_managed_tree(&self.data_root, &path)?;
        }
        Ok(reclaimed)
    }

    fn load_metadata(&self) -> Result<Vec<ProfileMetadata>> {
        read_metadata(&self.data_root.join("profiles.json"))
    }

    fn save_metadata(&self, profiles: &[ProfileMetadata]) -> Result<()> {
        atomic_write_metadata(&self.data_root, profiles)
    }

    fn profile_paths(&self, id: &str) -> Result<ProfilePaths> {
        validate_id(id)?;
        let base = self.data_root.join("profiles").join(id);
        if base == self.global_codex_home || base.starts_with(&self.global_codex_home) {
            return Err("Refusing to use the default Codex home".to_string());
        }
        Ok(ProfilePaths {
            codex_home: base.join("codex-home"),
            vscode_home: base.join("vscode-user-data"),
            extensions_dir: base.join("vscode-extensions"),
        })
    }

    pub(crate) fn vscode_home(&self, id: &str) -> Result<PathBuf> {
        Ok(self.profile_paths(id)?.vscode_home)
    }

    fn is_running(&self, id: &str) -> Result<bool> {
        let paths = self.profile_paths(id)?;
        if profile_process_running(&paths.vscode_home) {
            return Ok(true);
        }
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        Ok(matches!(
            runtime.statuses.get(id),
            Some(RuntimeStatus::Running)
        ))
    }

    fn persist_profile_credential(&self, id: &str, credential: &str) -> Result<()> {
        validate_id(id)?;
        validate_auth_structure(credential)?;
        let paths = self.profile_paths(id)?;
        ensure_private_managed_dir(&self.data_root, &paths.codex_home)?;
        write_codex_config(&paths.codex_home)?;
        let auth_path = paths.codex_home.join("auth.json");
        match fs::symlink_metadata(&auth_path) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
                return Err("The isolated profile credential is not a regular file".to_string());
            }
            Ok(_) | Err(_) => {}
        }
        write_private_file(&auth_path, credential.as_bytes())
    }

    fn read_persisted_profile_credential(&self, id: &str) -> Result<Option<String>> {
        validate_id(id)?;
        let auth_path = self.profile_paths(id)?.codex_home.join("auth.json");
        match fs::symlink_metadata(&auth_path) {
            Ok(_) => read_persisted_credential(&auth_path).map(Some),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err("Could not inspect the isolated profile credential".to_string()),
        }
    }

    fn current_profile_credential(&self, id: &str) -> Result<String> {
        if let Some(credential) = self.read_persisted_profile_credential(id)? {
            self.secrets.set(id, &credential)?;
            return Ok(credential);
        }
        let credential = self.secrets.get(id)?;
        self.persist_profile_credential(id, &credential)?;
        Ok(credential)
    }

    fn start_profile_monitor(
        &self,
        id: &str,
        vscode_home: PathBuf,
        auth_path: PathBuf,
    ) -> Result<()> {
        let should_start = {
            let mut runtime = self
                .runtime
                .lock()
                .map_err(|_| "Runtime state is unavailable")?;
            runtime.monitors.insert(id.to_string())
        };
        if !should_start {
            return Ok(());
        }

        let runtime = Arc::clone(&self.runtime);
        let secrets = Arc::clone(&self.secrets);
        let profile_id = id.to_string();
        std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(20);
            let mut detected = false;
            while Instant::now() < deadline {
                if profile_process_running(&vscode_home) {
                    detected = true;
                    break;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            if detected {
                while profile_process_running(&vscode_home) {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
            let sync = if detected {
                match read_persisted_credential(&auth_path) {
                    Ok(credential) => secrets.set(&profile_id, &credential),
                    Err(error) => Err(error),
                }
            } else {
                Err("VS Code did not start an isolated profile window. The saved credential was retained.".to_string())
            };
            if let Ok(mut state) = runtime.lock() {
                state.monitors.remove(&profile_id);
                if let Err(error) = sync {
                    state
                        .statuses
                        .insert(profile_id.clone(), RuntimeStatus::Error);
                    state.errors.insert(profile_id.clone(), error);
                } else {
                    state
                        .statuses
                        .insert(profile_id.clone(), RuntimeStatus::Idle);
                    state.errors.remove(&profile_id);
                }
            }
        });
        Ok(())
    }

    fn set_error(&self, id: &str, message: String) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime
                .statuses
                .insert(id.to_string(), RuntimeStatus::Error);
            runtime.errors.insert(id.to_string(), message);
        }
    }
}

fn account_tier_from_auth(auth_json: &str) -> Option<String> {
    let auth = serde_json::from_str::<Value>(auth_json).ok()?;
    let tokens = auth.get("tokens")?.as_object()?;
    for key in ["access_token", "id_token"] {
        let Some(token) = tokens.get(key).and_then(Value::as_str) else {
            continue;
        };
        let Some(payload) = token.split('.').nth(1) else {
            continue;
        };
        let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) else {
            continue;
        };
        let Ok(claims) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let Some(plan) = claims
            .get("https://api.openai.com/auth")
            .and_then(|auth| auth.get("chatgpt_plan_type"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let tier = match plan.to_ascii_lowercase().as_str() {
            "free" => "Free",
            "plus" => "Plus",
            "pro" => "Pro",
            "go" => "Go",
            "team" => "Team",
            "business" => "Business",
            "enterprise" => "Enterprise",
            _ => continue,
        };
        return Some(tier.to_string());
    }
    None
}

pub(crate) fn write_codex_config(codex_home: &Path) -> Result<()> {
    write_private_file(
        &codex_home.join("config.toml"),
        b"# Managed by Multi Codex. Keep each profile's Codex login isolated.\ncli_auth_credentials_store = \"file\"\n",
    )
}

fn canonical_workspace(workspace: &Path) -> Result<PathBuf> {
    let canonical = fs::canonicalize(workspace)
        .map_err(|_| "The selected workspace could not be opened".to_string())?;
    if !canonical.is_dir() {
        return Err("The selected workspace is not a directory".to_string());
    }
    Ok(canonical)
}

pub fn default_service() -> Result<ProfileService<KeyringSecretStore, CodexCliRecognizer>> {
    let home = dirs::home_dir().ok_or_else(|| "Home directory is unavailable".to_string())?;
    let data_root = dirs::data_dir()
        .ok_or_else(|| "Data directory is unavailable".to_string())?
        .join("multi-codex");
    let codex_home = crate::settings::global_codex_home(
        &home,
        env::var_os("CODEX_HOME"),
        &crate::settings::load()?,
    )?;
    ProfileService::new(
        data_root,
        codex_home,
        home.join(".vscode/extensions"),
        KeyringSecretStore,
        CodexCliRecognizer,
    )
}

pub fn validate_auth_structure(auth_json: &str) -> Result<String> {
    if auth_json.is_empty() {
        return Err("Auth JSON is required".to_string());
    }
    if auth_json.len() > MAX_AUTH_BYTES {
        return Err("Auth JSON must be smaller than 1 MiB".to_string());
    }
    let value: Value =
        serde_json::from_str(auth_json).map_err(|_| "Auth JSON is not valid JSON".to_string())?;
    let object = value
        .as_object()
        .ok_or_else(|| "Auth JSON must contain a JSON object".to_string())?;
    let mode = object
        .get("auth_mode")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "Auth JSON is missing a recognized auth_mode".to_string())?;
    let has_api_key = object
        .get("OPENAI_API_KEY")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.is_empty());
    let has_token = object
        .get("tokens")
        .and_then(Value::as_object)
        .is_some_and(|tokens| {
            ["access_token", "refresh_token", "id_token"]
                .iter()
                .any(|key| {
                    tokens
                        .get(*key)
                        .and_then(Value::as_str)
                        .is_some_and(|value| !value.is_empty())
                })
        });
    if !has_api_key && !has_token {
        return Err("Auth JSON does not contain a Codex credential".to_string());
    }
    Ok(display_auth_mode(mode))
}

fn display_auth_mode(mode: &str) -> String {
    match mode.to_ascii_lowercase().as_str() {
        "chatgpt" => "ChatGPT".to_string(),
        "apikey" | "api_key" => "API key".to_string(),
        _ => mode.to_string(),
    }
}

fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err("Profile name must be between 1 and 64 characters".to_string());
    }
    if name.chars().any(char::is_control) {
        return Err("Profile name contains unsupported characters".to_string());
    }
    Ok(name.to_string())
}

fn validate_notes(notes: Option<String>) -> Result<Option<String>> {
    let notes = notes
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if notes
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err("Notes must be 500 characters or fewer".to_string());
    }
    Ok(notes)
}

fn validate_id(id: &str) -> Result<()> {
    match Uuid::parse_str(id) {
        Ok(parsed) if parsed.to_string() == id => Ok(()),
        _ => Err("Invalid profile identifier".to_string()),
    }
}

fn ensure_unique_name(
    profiles: &[ProfileMetadata],
    name: &str,
    except_id: Option<&str>,
) -> Result<()> {
    if profiles.iter().any(|profile| {
        Some(profile.id.as_str()) != except_id && profile.name.eq_ignore_ascii_case(name)
    }) {
        Err("A profile with this name already exists".to_string())
    } else {
        Ok(())
    }
}

fn read_metadata(path: &Path) -> Result<Vec<ProfileMetadata>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let bytes =
        fs::read(path).map_err(|error| format!("Could not read profile metadata: {error}"))?;
    serde_json::from_slice(&bytes).map_err(|error| format!("Profile metadata is invalid: {error}"))
}

fn atomic_write_metadata(root: &Path, profiles: &[ProfileMetadata]) -> Result<()> {
    ensure_private_dir(root)?;
    let temp_path = root.join(format!(".profiles-{}.tmp", Uuid::new_v4()));
    let destination = root.join("profiles.json");
    let bytes = serde_json::to_vec_pretty(profiles)
        .map_err(|error| format!("Could not encode profile metadata: {error}"))?;
    write_private_file(&temp_path, &bytes)?;
    fs::rename(&temp_path, &destination)
        .map_err(|error| format!("Could not save profile metadata: {error}"))?;
    File::open(root)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("Could not sync profile metadata: {error}"))?;
    Ok(())
}

fn ensure_private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)
        .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    set_owner_only_dir(path)
}

fn directory_size(path: &Path) -> Result<u64> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("Could not inspect app storage: {error}"))?;
    if metadata.file_type().is_symlink() {
        return Ok(0);
    }
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }
    let mut total = 0_u64;
    for entry in
        fs::read_dir(path).map_err(|error| format!("Could not read app storage: {error}"))?
    {
        let entry = entry.map_err(|error| format!("Could not read app storage: {error}"))?;
        total = total.saturating_add(directory_size(&entry.path())?);
    }
    Ok(total)
}

fn reclaimable_size(profile_root: &Path) -> Result<u64> {
    let mut total = 0_u64;
    for components in RECLAIMABLE_PATHS {
        let path = components
            .iter()
            .fold(profile_root.to_path_buf(), |path, component| {
                path.join(component)
            });
        match fs::symlink_metadata(&path) {
            Ok(_) => total = total.saturating_add(directory_size(&path)?),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Could not inspect app storage: {error}")),
        }
    }
    Ok(total)
}

pub(crate) fn set_owner_only_dir(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Could not protect {}: {error}", path.display()))
}

fn set_owner_only_file(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect {}: {error}", path.display()))
}

fn read_persisted_credential(path: &Path) -> Result<String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| "Could not inspect the isolated profile credential".to_string())?;
    if !metadata.file_type().is_file() || metadata.file_type().is_symlink() {
        return Err("The isolated profile credential is not a regular file".to_string());
    }
    if metadata.len() as usize > MAX_AUTH_BYTES {
        return Err("The isolated profile credential is too large".to_string());
    }
    set_owner_only_file(path)?;
    let credential = fs::read_to_string(path)
        .map_err(|_| "Could not read the isolated profile credential".to_string())?;
    validate_auth_structure(&credential)?;
    Ok(credential)
}

fn ensure_private_managed_dir(root: &Path, path: &Path) -> Result<()> {
    ensure_private_dir(root)?;
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    if path == root || !path.starts_with(root) {
        return Err("Refusing to access a path outside Multi Codex data".to_string());
    }
    let relative = path
        .strip_prefix(root)
        .map_err(|_| "Invalid managed profile path".to_string())?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
                return Err("Refusing to access a linked or invalid profile directory".to_string());
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                fs::create_dir(&current)
                    .map_err(|error| format!("Could not create profile directory: {error}"))?;
            }
            Err(error) => return Err(format!("Could not inspect profile directory: {error}")),
        }
        set_owner_only_dir(&current)?;
        let canonical_current = fs::canonicalize(&current)
            .map_err(|error| format!("Could not resolve profile directory: {error}"))?;
        if canonical_current == canonical_root || !canonical_current.starts_with(&canonical_root) {
            return Err("Refusing to access a path outside Multi Codex data".to_string());
        }
    }
    Ok(())
}

pub(crate) fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        ensure_private_dir(parent)?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .mode(0o600)
        .open(path)
        .map_err(|error| format!("Could not create protected file: {error}"))?;
    file.set_permissions(fs::Permissions::from_mode(0o600))
        .map_err(|error| format!("Could not protect file: {error}"))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("Could not write protected file: {error}"))
}

fn remove_private_file(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("Could not remove isolated credential: {error}")),
    }
}

fn remove_managed_tree(root: &Path, path: &Path) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
    let canonical_path = fs::canonicalize(path)
        .map_err(|error| format!("Could not resolve profile directory: {error}"))?;
    if canonical_path == canonical_root || !canonical_path.starts_with(&canonical_root) {
        return Err("Refusing to delete a path outside Multi Codex data".to_string());
    }
    fs::remove_dir_all(canonical_path)
        .map_err(|error| format!("Could not delete isolated profile data: {error}"))
}

// VS Code can reuse an already-open folder even with --new-window. A separate
// single-folder workspace gives every launch its own window identity. Keep the
// descriptor for VS Code session restore; project files and profile homes stay put.
pub(crate) fn create_launch_workspace(
    data_root: &Path,
    profile_root: &Path,
    workspace: &Path,
) -> Result<PathBuf> {
    let directory = profile_root
        .join("launch-workspaces")
        .join(Uuid::new_v4().to_string());
    ensure_private_managed_dir(data_root, &directory)?;
    let mut name = workspace
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Workspace")
        .to_string();
    while name.len() > 160 {
        name.pop();
    }
    let path = directory.join(format!("{name}.code-workspace"));
    let contents = serde_json::to_vec_pretty(&serde_json::json!({
        "folders": [{ "path": workspace.to_str().ok_or("Workspace path is not valid UTF-8")? }]
    }))
    .map_err(|error| error.to_string())?;
    write_private_file(&path, &contents)?;
    Ok(path)
}

fn build_vscode_command_with(
    code: &Path,
    codex_home: &Path,
    vscode_home: &Path,
    extensions_dir: &Path,
    workspace: &Path,
) -> Command {
    let mut command = Command::new(code);
    command
        .arg("--new-window")
        .arg("--user-data-dir")
        .arg(vscode_home)
        .arg("--extensions-dir")
        .arg(extensions_dir)
        .arg(workspace)
        .env("CODEX_HOME", codex_home)
        .env_remove("VSCODE_IPC_HOOK_CLI")
        .env_remove("VSCODE_PID")
        .env_remove("VSCODE_CWD")
        .env_remove("ELECTRON_RUN_AS_NODE");
    command
}

pub(crate) fn resolve_command(name: &str) -> Result<PathBuf> {
    let settings = crate::settings::load()?;
    let configured = match name {
        "code" => settings.code_path,
        "codex" => settings.codex_path,
        _ => None,
    };
    if let Some(path) = configured {
        let path = PathBuf::from(path);
        if is_executable_file(&path) {
            return Ok(path);
        }
        return Err(format!(
            "The saved {name} executable is unavailable. Update its path in Launch settings."
        ));
    }
    let home = dirs::home_dir().unwrap_or_default();
    resolve_command_with(name, env::var_os("PATH").as_deref(), &home).ok_or_else(|| {
        format!("{name} was not found. Install it or set its executable path in Launch settings.")
    })
}

pub(crate) fn resolve_codex_command() -> Result<PathBuf> {
    resolve_command("codex")
}

#[cfg(test)]
fn require_codex_command(path: Option<&OsStr>, home: &Path) -> Result<PathBuf> {
    resolve_command_with("codex", path, home).ok_or_else(|| {
        "Codex CLI was not found. Install Codex or the OpenAI VS Code extension, then reopen Multi Codex".to_string()
    })
}

fn resolve_command_with(name: &str, path: Option<&OsStr>, home: &Path) -> Option<PathBuf> {
    if let Some(candidate) = path.and_then(|paths| {
        env::split_paths(paths)
            .map(|directory| directory.join(name))
            .find(|candidate| is_executable_file(candidate))
    }) {
        return Some(candidate);
    }

    if name == "codex" {
        let candidates = [
            Some(home.join(".local/bin/codex")),
            find_extension_codex(home),
        ];
        if let Some(candidate) = candidates
            .into_iter()
            .flatten()
            .find(|candidate| is_executable_file(candidate))
        {
            return Some(candidate);
        }
    }

    #[cfg(target_os = "macos")]
    {
        let candidates = match name {
            "code" => vec![
                PathBuf::from(
                    "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
                ),
                home.join("Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"),
            ],
            "codex" => vec![
                PathBuf::from("/opt/homebrew/bin/codex"),
                PathBuf::from("/usr/local/bin/codex"),
            ],
            _ => Vec::new(),
        };
        if let Some(path) = candidates
            .into_iter()
            .find(|candidate| is_executable_file(candidate))
        {
            return Some(path);
        }
    }

    None
}

pub(crate) fn is_executable_file(path: &Path) -> bool {
    fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

fn find_extension_codex(home: &Path) -> Option<PathBuf> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    const PLATFORM: &str = "linux-x86_64";
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    const PLATFORM: &str = "linux-aarch64";
    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    const PLATFORM: &str = "darwin-x86_64";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const PLATFORM: &str = "darwin-arm64";
    #[cfg(not(any(
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
        all(target_os = "macos", target_arch = "x86_64"),
        all(target_os = "macos", target_arch = "aarch64")
    )))]
    const PLATFORM: &str = "unsupported";

    find_extension_codex_for_platform(home, PLATFORM)
}

fn find_extension_codex_for_platform(home: &Path, platform: &str) -> Option<PathBuf> {
    let extensions = home.join(".vscode/extensions");
    let mut versions = fs::read_dir(&extensions)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("openai.chatgpt-")
        })
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    versions.sort();
    versions
        .into_iter()
        .rev()
        .map(|extension| extension.join("bin").join(platform).join("codex"))
        .find(|candidate| is_executable_file(candidate))
}

fn arguments_use_profile(arguments: &[Vec<u8>], vscode_home: &Path) -> bool {
    let expected = vscode_home.as_os_str().as_encoded_bytes();
    // Electron can rewrite argv and leave the user-data path as a standalone
    // argument. Match whole arguments, never substrings or a flattened ps line.
    arguments.iter().any(|arg| {
        arg == expected
            || arg
                .strip_prefix(b"--user-data-dir=")
                .is_some_and(|value| value == expected)
    })
}

pub(crate) fn process_uses_profile(pid: u32, vscode_home: &Path) -> bool {
    if process_arguments(pid).is_some_and(|args| arguments_use_profile(&args, vscode_home)) {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        profile_data_open_by_process(pid, vscode_home, Path::new("/proc"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

pub(crate) fn window_process_uses_profile(pid: u32, vscode_home: &Path) -> bool {
    if process_uses_profile(pid, vscode_home) {
        return true;
    }
    #[cfg(target_os = "linux")]
    {
        // Wayland may attribute an Electron surface to its GPU child. Only walk
        // ancestors of a VS Code executable, never an arbitrary application's PID.
        if !is_vscode_process(&Path::new("/proc").join(pid.to_string())) {
            return false;
        }
        let mut current = pid;
        let mut seen = std::collections::HashSet::new();
        for _ in 0..16 {
            if current <= 1 || !seen.insert(current) {
                break;
            }
            let Some(parent) = fs::read_to_string(format!("/proc/{current}/status"))
                .ok()
                .and_then(|status| {
                    status
                        .lines()
                        .find_map(|line| line.strip_prefix("PPid:")?.trim().parse::<u32>().ok())
                })
            else {
                break;
            };
            if process_uses_profile(parent, vscode_home) {
                return true;
            }
            current = parent;
        }
    }
    false
}

#[cfg(target_os = "linux")]
fn is_vscode_process(process: &Path) -> bool {
    fs::read_link(process.join("exe"))
        .ok()
        .and_then(|exe| {
            exe.file_name()
                .map(|name| name.to_string_lossy().to_ascii_lowercase())
        })
        .is_some_and(|name| {
            matches!(
                name.as_str(),
                "code" | "code-insiders" | "code-oss" | "codium" | "vscodium"
            )
        })
}

#[cfg(target_os = "linux")]
fn profile_data_open_by_process(pid: u32, vscode_home: &Path, proc_root: &Path) -> bool {
    let process = proc_root.join(pid.to_string());
    if !is_vscode_process(&process) {
        return false;
    }
    let Ok(home) = vscode_home.canonicalize() else {
        return false;
    };
    let locks = [
        home.join("Local Storage/leveldb/LOCK"),
        home.join("Service Worker/Database/LOCK"),
    ];
    // Electron's process title can replace every argv boundary. Its open profile
    // database locks still identify the user-data directory exactly; never parse
    // that flattened title or use a substring to guess which profile owns it.
    fs::read_dir(process.join("fd"))
        .ok()
        .is_some_and(|entries| {
            entries.flatten().any(|entry| {
                fs::read_link(entry.path())
                    .ok()
                    .is_some_and(|path| locks.contains(&path))
            })
        })
}

#[cfg(target_os = "linux")]
fn process_arguments(pid: u32) -> Option<Vec<Vec<u8>>> {
    Some(
        fs::read(format!("/proc/{pid}/cmdline"))
            .ok()?
            .split(|b| *b == 0)
            .filter(|a| !a.is_empty())
            .map(|a| a.to_vec())
            .collect(),
    )
}

#[cfg(target_os = "macos")]
fn process_arguments(pid: u32) -> Option<Vec<Vec<u8>>> {
    // KERN_PROCARGS2 preserves argument boundaries, including spaces and Unicode.
    let mut mib = [
        libc::CTL_KERN,
        libc::KERN_PROCARGS2,
        i32::try_from(pid).ok()?,
    ];
    let mut size = 0usize;
    // SAFETY: mib has three initialized integers; size is writable; this first
    // call only asks for the required buffer length.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
        || size > 4 * 1024 * 1024
    {
        return None;
    }
    let mut bytes = vec![0u8; size];
    // SAFETY: bytes is allocated to size bytes and sysctl cannot write beyond it.
    if unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            bytes.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    } != 0
    {
        return None;
    }
    bytes.truncate(size);
    parse_macos_arguments(&bytes)
}

#[cfg(any(target_os = "macos", test))]
fn parse_macos_arguments(bytes: &[u8]) -> Option<Vec<Vec<u8>>> {
    let count = i32::from_ne_bytes(bytes.get(..4)?.try_into().ok()?);
    if !(1..=65536).contains(&count) {
        return None;
    }
    let mut offset = 4 + bytes.get(4..)?.iter().position(|b| *b == 0)? + 1;
    while bytes.get(offset) == Some(&0) {
        offset += 1;
    }
    let mut arguments = Vec::new();
    for _ in 0..count {
        let remaining = bytes.get(offset..)?;
        let end = remaining.iter().position(|b| *b == 0)?;
        arguments.push(remaining[..end].to_vec());
        offset += end + 1;
    }
    Some(arguments)
}

#[cfg(target_os = "linux")]
fn profile_process_running(vscode_home: &Path) -> bool {
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().to_str()?.parse::<u32>().ok())
        .any(|pid| process_uses_profile(pid, vscode_home))
}

#[cfg(target_os = "macos")]
fn profile_process_running(vscode_home: &Path) -> bool {
    let Ok(output) = crate::process::output(
        Command::new("/bin/ps").args(["-axo", "pid="]),
        Duration::from_secs(3),
    ) else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .any(|pid| process_uses_profile(pid, vscode_home))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[cfg(target_os = "linux")]
    fn identifies_rewritten_electron_titles_by_exact_open_profile_locks() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let profile = root.path().join("Profile A 工具/vscode-user-data");
        fs::create_dir_all(profile.join("Local Storage/leveldb")).unwrap();
        let lock = profile.join("Local Storage/leveldb/LOCK");
        fs::write(&lock, "").unwrap();
        let proc_root = root.path().join("proc");
        let process = proc_root.join("42");
        fs::create_dir_all(process.join("fd")).unwrap();
        symlink("/usr/share/code/code", process.join("exe")).unwrap();
        symlink(&lock, process.join("fd/8")).unwrap();
        assert!(profile_data_open_by_process(42, &profile, &proc_root));
        let other = root.path().join("Profile A 工具 copy/vscode-user-data");
        fs::create_dir_all(&other).unwrap();
        assert!(!profile_data_open_by_process(42, &other, &proc_root));
        fs::remove_file(process.join("fd/8")).unwrap();
        symlink(profile.join("notes.txt"), process.join("fd/8")).unwrap();
        assert!(!profile_data_open_by_process(42, &profile, &proc_root));
        fs::remove_file(process.join("fd/8")).unwrap();
        symlink(&lock, process.join("fd/8")).unwrap();
        fs::remove_file(process.join("exe")).unwrap();
        symlink("/usr/bin/backup-tool", process.join("exe")).unwrap();
        assert!(!profile_data_open_by_process(42, &profile, &proc_root));
    }

    #[test]
    fn process_arguments_match_whole_paths_and_do_not_match_other_profiles() {
        let home = Path::new("/Users/A B/工具/vscode-user-data");
        assert!(arguments_use_profile(
            &[home.as_os_str().as_encoded_bytes().to_vec()],
            home
        ));
        assert!(arguments_use_profile(
            &[format!("--user-data-dir={}", home.display()).into_bytes()],
            home
        ));
        assert!(!arguments_use_profile(
            &[format!("{}-other", home.display()).into_bytes()],
            home
        ));
        assert!(!arguments_use_profile(
            &[format!("prefix {} suffix", home.display()).into_bytes()],
            home
        ));
    }
    #[test]
    fn macos_argument_parser_preserves_spaces_and_excludes_environment() {
        let mut raw = 3i32.to_ne_bytes().to_vec();
        raw.extend_from_slice(
            b"/Applications/Code\0\0code\0--user-data-dir\0/Users/A B/profile\0SECRET=value\0",
        );
        let args = parse_macos_arguments(&raw).unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[2], b"/Users/A B/profile");
        assert!(parse_macos_arguments(&raw[..5]).is_none());
    }

    use std::collections::HashMap;
    use std::os::unix::fs::symlink;
    use std::sync::Mutex;
    use tempfile::TempDir;

    #[derive(Default)]
    struct MemorySecrets(Mutex<HashMap<String, String>>);

    impl SecretStore for MemorySecrets {
        fn set(&self, id: &str, secret: &str) -> Result<()> {
            self.0
                .lock()
                .unwrap()
                .insert(id.to_string(), secret.to_string());
            Ok(())
        }
        fn get(&self, id: &str) -> Result<String> {
            self.0
                .lock()
                .unwrap()
                .get(id)
                .cloned()
                .ok_or_else(|| "missing secret".to_string())
        }
        fn delete(&self, id: &str) -> Result<()> {
            self.0.lock().unwrap().remove(id);
            Ok(())
        }
    }

    struct AcceptAuth;
    impl AuthRecognizer for AcceptAuth {
        fn recognize(&self, _auth_json: &str) -> Result<()> {
            Ok(())
        }
    }

    fn fixture() -> (TempDir, ProfileService<MemorySecrets, AcceptAuth>) {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("data/multi-codex");
        let service = ProfileService::new(
            root,
            temp.path().join("home/.codex"),
            temp.path().join("home/.vscode/extensions"),
            MemorySecrets::default(),
            AcceptAuth,
        )
        .unwrap();
        (temp, service)
    }

    fn sample_auth() -> String {
        r#"{"auth_mode":"chatgpt","tokens":{"access_token":"test-only"}}"#.to_string()
    }

    fn sample_input(name: &str) -> SaveProfileInput {
        SaveProfileInput {
            name: name.into(),
            auth_json: sample_auth(),
            notes: None,
        }
    }

    fn write_executable(path: &Path, contents: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[test]
    fn validates_supported_shapes_without_exposing_values() {
        assert_eq!(validate_auth_structure(&sample_auth()).unwrap(), "ChatGPT");
        assert!(validate_auth_structure("[]").is_err());
        assert!(validate_auth_structure(r#"{"auth_mode":"chatgpt"}"#).is_err());
    }

    #[test]
    fn derives_a_supported_chatgpt_plan_without_exposing_the_credential() {
        let claims = serde_json::json!({
            "https://api.openai.com/auth": { "chatgpt_plan_type": "plus" }
        });
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).unwrap());
        let auth = format!(
            r#"{{"auth_mode":"chatgpt","tokens":{{"access_token":"header.{payload}.signature"}}}}"#
        );
        assert_eq!(account_tier_from_auth(&auth).as_deref(), Some("Plus"));
        assert_eq!(
            account_tier_from_auth(r#"{"auth_mode":"chatgpt","tokens":{}}"#),
            None
        );
    }

    #[test]
    fn rejects_traversal_identifiers_and_outside_deletes() {
        let (temp, service) = fixture();
        assert!(service.profile_paths("../../.codex").is_err());
        let outside = temp.path().join("outside");
        fs::create_dir_all(&outside).unwrap();
        assert!(remove_managed_tree(&service.data_root, &outside).is_err());
        assert!(outside.exists());
    }

    #[test]
    fn metadata_and_directories_are_owner_only() {
        let (_temp, service) = fixture();
        service.add_profile(sample_input("Personal")).unwrap();
        let metadata = service.data_root.join("profiles.json");
        assert_eq!(
            fs::metadata(&metadata).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&service.data_root)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
        assert!(!fs::read_dir(&service.data_root)
            .unwrap()
            .flatten()
            .any(|entry| entry.file_name().to_string_lossy().ends_with(".tmp")));
    }

    #[test]
    fn storage_usage_counts_managed_files_without_following_symlinks() {
        let (temp, service) = fixture();
        let first = service.data_root.join("profiles/first.bin");
        let nested = service.data_root.join("profiles/nested/second.bin");
        fs::create_dir_all(nested.parent().unwrap()).unwrap();
        fs::write(&first, vec![0_u8; 11]).unwrap();
        fs::write(&nested, vec![0_u8; 17]).unwrap();
        let outside = temp.path().join("outside.bin");
        fs::write(&outside, vec![0_u8; 1_000]).unwrap();
        symlink(&outside, service.data_root.join("profiles/outside-link")).unwrap();

        assert_eq!(service.storage_usage().unwrap().bytes, 28);
    }

    #[test]
    fn storage_breakdown_reports_profile_and_reclaimable_bytes() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let root = service
            .data_root
            .join("profiles")
            .join(&profile.metadata.id);
        let cache = root.join("vscode-user-data/CachedExtensionVSIXs");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("cached.vsix"), vec![0_u8; 31]).unwrap();
        let usage = service.storage_usage().unwrap();
        let item = usage
            .profiles
            .iter()
            .find(|item| item.id == profile.metadata.id)
            .unwrap();
        assert_eq!(item.reclaimable_bytes, 31);
        assert!(item.bytes >= item.reclaimable_bytes);
        assert_eq!(usage.reclaimable_bytes, 31);
    }

    #[test]
    fn cache_cleanup_preserves_credentials_sessions_settings_and_extensions() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let root = service
            .data_root
            .join("profiles")
            .join(&profile.metadata.id);
        let cache = root.join("vscode-user-data/CachedExtensionVSIXs");
        let settings = root.join("vscode-user-data/User/settings.json");
        let extension = root.join("vscode-extensions/openai.chatgpt-test/extension.js");
        let session = root.join("codex-home/sessions/2026/session.jsonl");
        fs::create_dir_all(&cache).unwrap();
        fs::create_dir_all(settings.parent().unwrap()).unwrap();
        fs::create_dir_all(extension.parent().unwrap()).unwrap();
        fs::create_dir_all(session.parent().unwrap()).unwrap();
        fs::write(cache.join("cached.vsix"), vec![0_u8; 31]).unwrap();
        fs::write(&settings, "settings").unwrap();
        fs::write(&extension, "extension").unwrap();
        fs::write(&session, "session").unwrap();

        assert_eq!(
            service.clear_profile_cache(&profile.metadata.id).unwrap(),
            31
        );
        assert!(!cache.exists());
        assert!(settings.exists());
        assert!(extension.exists());
        assert!(session.exists());
        assert!(root.join("codex-home/auth.json").exists());
    }

    #[test]
    fn cache_cleanup_refuses_symlink_escapes() {
        let (temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let root = service
            .data_root
            .join("profiles")
            .join(&profile.metadata.id);
        let outside = temp.path().join("outside-cache");
        fs::create_dir_all(&outside).unwrap();
        fs::write(outside.join("keep"), "safe").unwrap();
        let cache = root.join("vscode-user-data/CachedExtensionVSIXs");
        fs::create_dir_all(cache.parent().unwrap()).unwrap();
        symlink(&outside, &cache).unwrap();

        assert!(service.clear_profile_cache(&profile.metadata.id).is_err());
        assert!(outside.join("keep").exists());
    }

    #[test]
    fn cache_cleanup_refuses_running_profiles() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        service
            .runtime
            .lock()
            .unwrap()
            .statuses
            .insert(profile.metadata.id.clone(), RuntimeStatus::Running);
        assert_eq!(
            service
                .clear_profile_cache(&profile.metadata.id)
                .unwrap_err(),
            "Close this profile's VS Code windows before clearing its cache"
        );
    }

    #[test]
    fn credential_is_materialized_privately_and_persists_between_launches() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let codex_home = service
            .profile_paths(&profile.metadata.id)
            .unwrap()
            .codex_home;
        let auth_path = codex_home.join("auth.json");
        assert_eq!(fs::read_to_string(&auth_path).unwrap(), sample_auth());
        assert_eq!(
            fs::metadata(&auth_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            service
                .current_profile_credential(&profile.metadata.id)
                .unwrap(),
            sample_auth()
        );
        assert!(auth_path.exists());
    }

    #[test]
    fn persisted_credential_is_preferred_and_synchronized_to_the_keyring() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let auth_path = service
            .profile_paths(&profile.metadata.id)
            .unwrap()
            .codex_home
            .join("auth.json");
        let refreshed = r#"{"auth_mode":"chatgpt","tokens":{"access_token":"refreshed"}}"#;
        write_private_file(&auth_path, refreshed.as_bytes()).unwrap();

        assert_eq!(
            service
                .current_profile_credential(&profile.metadata.id)
                .unwrap(),
            refreshed
        );
        assert_eq!(
            service.secrets.get(&profile.metadata.id).unwrap(),
            refreshed
        );
    }

    #[test]
    fn browser_reauthentication_replaces_only_the_existing_profiles_credential() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let pending = service
            .prepare_profile_reauthentication(&profile.metadata.id)
            .unwrap();
        let replacement =
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"fresh-browser-session"}}"#;
        write_private_file(
            &pending.codex_home.join("auth.json"),
            replacement.as_bytes(),
        )
        .unwrap();

        let updated = service.finish_device_login(pending).unwrap();

        assert_eq!(updated.metadata.id, profile.metadata.id);
        assert_eq!(updated.metadata.name, profile.metadata.name);
        assert_eq!(service.load_metadata().unwrap().len(), 1);
        assert_eq!(
            service.secrets.get(&profile.metadata.id).unwrap(),
            replacement
        );
        assert_eq!(
            service
                .read_persisted_profile_credential(&profile.metadata.id)
                .unwrap()
                .as_deref(),
            Some(replacement)
        );
    }

    #[test]
    fn updating_a_profile_replaces_the_persisted_credential() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let replacement = r#"{"auth_mode":"chatgpt","tokens":{"access_token":"replacement"}}"#;
        service
            .update_profile(
                &profile.metadata.id,
                "Work".into(),
                Some(replacement.into()),
                None,
            )
            .unwrap();

        let auth_path = service
            .profile_paths(&profile.metadata.id)
            .unwrap()
            .codex_home
            .join("auth.json");
        assert_eq!(fs::read_to_string(auth_path).unwrap(), replacement);
        assert_eq!(
            service.secrets.get(&profile.metadata.id).unwrap(),
            replacement
        );
    }

    #[test]
    fn repeat_launches_use_new_windows_and_keep_the_profile_credential() {
        let (temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let workspace = temp.path().join("workspace 工具 with spaces");
        fs::create_dir(&workspace).unwrap();
        let launcher = temp.path().join("bin/code");
        write_executable(
            &launcher,
            b"#!/bin/sh\nfor arg; do last=\"$arg\"; done\nprintf '%s\\n' \"$last\" >> \"$(dirname \"$0\")/launches\"\nsh -c 'sleep 1' multi-codex-profile \"$@\" &\n",
        );

        service
            .launch_profile_with_command(&profile.metadata.id, &workspace, &launcher)
            .unwrap();
        service
            .launch_profile_with_command(&profile.metadata.id, &workspace, &launcher)
            .unwrap();

        let launches = temp.path().join("bin/launches");
        let deadline = Instant::now() + Duration::from_secs(2);
        let requests = loop {
            let requests = fs::read_to_string(&launches).unwrap_or_default();
            if requests.lines().count() == 2 {
                break requests;
            }
            assert!(
                Instant::now() < deadline,
                "the fake launcher did not receive both window requests"
            );
            std::thread::sleep(Duration::from_millis(20));
        };
        assert_eq!(requests.lines().count(), 2);
        let descriptors: Vec<_> = requests.lines().map(PathBuf::from).collect();
        assert_ne!(descriptors[0], descriptors[1]);
        for descriptor in descriptors {
            assert_eq!(descriptor.extension().unwrap(), "code-workspace");
            let contents: serde_json::Value =
                serde_json::from_slice(&fs::read(&descriptor).unwrap()).unwrap();
            assert_eq!(
                contents["folders"][0]["path"],
                workspace.canonicalize().unwrap().to_str().unwrap()
            );
            assert_eq!(
                fs::metadata(&descriptor).unwrap().permissions().mode() & 0o777,
                0o600
            );
            assert_eq!(
                fs::metadata(descriptor.parent().unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
        }
        assert_eq!(fs::read_dir(&workspace).unwrap().count(), 0);

        std::thread::sleep(Duration::from_millis(1200));
        let auth_path = service
            .profile_paths(&profile.metadata.id)
            .unwrap()
            .codex_home
            .join("auth.json");
        assert_eq!(fs::read_to_string(auth_path).unwrap(), sample_auth());
    }

    #[test]
    fn failed_launch_keeps_the_persisted_profile_credential() {
        let (temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let missing_launcher = temp.path().join("missing-code");

        assert!(service
            .launch_profile_with_command(&profile.metadata.id, &workspace, &missing_launcher)
            .is_err());
        let auth_path = service
            .profile_paths(&profile.metadata.id)
            .unwrap()
            .codex_home
            .join("auth.json");
        assert_eq!(fs::read_to_string(auth_path).unwrap(), sample_auth());
        let launch_root = service
            .data_root
            .join("profiles")
            .join(&profile.metadata.id)
            .join("launch-workspaces");
        assert_eq!(fs::read_dir(launch_root).unwrap().count(), 0);
    }

    #[test]
    fn launch_workspaces_refuse_symlink_escapes() {
        let (temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let profile_root = service
            .data_root
            .join("profiles")
            .join(&profile.metadata.id);
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        symlink(&outside, profile_root.join("launch-workspaces")).unwrap();
        assert!(create_launch_workspace(&service.data_root, &profile_root, &outside).is_err());
        assert_eq!(fs::read_dir(outside).unwrap().count(), 0);
    }

    #[test]
    fn delete_removes_only_selected_profile_data_and_secret() {
        let (_temp, service) = fixture();
        let first = service.add_profile(sample_input("First")).unwrap();
        let second = service.add_profile(sample_input("Second")).unwrap();
        let first_home = service
            .profile_paths(&first.metadata.id)
            .unwrap()
            .codex_home;
        let second_home = service
            .profile_paths(&second.metadata.id)
            .unwrap()
            .codex_home;
        ensure_private_managed_dir(&service.data_root, &first_home).unwrap();
        ensure_private_managed_dir(&service.data_root, &second_home).unwrap();
        service.delete_profile(&first.metadata.id).unwrap();
        assert!(!first_home.exists());
        assert!(second_home.exists());
        assert!(service.secrets.get(&first.metadata.id).is_err());
        assert!(service.secrets.get(&second.metadata.id).is_ok());
    }

    #[test]
    fn launch_arguments_keep_state_isolated() {
        let temp = tempfile::tempdir().unwrap();
        let codex = temp.path().join("codex");
        let vscode = temp.path().join("vscode");
        let extensions = temp.path().join("extensions");
        let workspace = temp.path().join("workspace");
        let command = build_vscode_command_with(&codex, &codex, &vscode, &extensions, &workspace);
        let args: Vec<_> = command.get_args().map(|value| value.to_owned()).collect();
        assert!(args.iter().any(|argument| argument == "--new-window"));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--user-data-dir" && pair[1] == vscode.as_os_str()));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--extensions-dir" && pair[1] == extensions.as_os_str()));
        assert!(args
            .iter()
            .any(|argument| argument == workspace.as_os_str()));
        assert!(!args.iter().any(|argument| argument == "--wait"));
        assert_eq!(
            command
                .get_envs()
                .find(|(key, _)| *key == "CODEX_HOME")
                .unwrap()
                .1
                .unwrap(),
            codex.as_os_str()
        );
        for key in [
            "VSCODE_IPC_HOOK_CLI",
            "VSCODE_PID",
            "VSCODE_CWD",
            "ELECTRON_RUN_AS_NODE",
        ] {
            assert!(command
                .get_envs()
                .any(|(name, value)| name == key && value.is_none()));
        }
    }

    #[test]
    fn profile_config_forces_file_based_credentials() {
        let temp = tempfile::tempdir().unwrap();
        write_codex_config(temp.path()).unwrap();
        assert_eq!(
            fs::read_to_string(temp.path().join("config.toml")).unwrap(),
            "# Managed by Multi Codex. Keep each profile's Codex login isolated.\ncli_auth_credentials_store = \"file\"\n"
        );
    }

    #[test]
    fn finds_codex_bundled_with_the_newest_vscode_extension() {
        let temp = tempfile::tempdir().unwrap();
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        let platform = "linux-x86_64";
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        let platform = "linux-aarch64";
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        let platform = "darwin-x86_64";
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        let platform = "darwin-arm64";
        let older = temp
            .path()
            .join(".vscode/extensions/openai.chatgpt-1.0.0/bin")
            .join(platform)
            .join("codex");
        let newest = temp
            .path()
            .join(".vscode/extensions/openai.chatgpt-2.0.0/bin")
            .join(platform)
            .join("codex");
        write_executable(&older, b"old");
        write_executable(&newest, b"new");
        assert_eq!(find_extension_codex(temp.path()), Some(newest));
    }

    #[test]
    fn resolves_extension_codex_when_desktop_path_is_restricted() {
        let temp = tempfile::tempdir().unwrap();
        let path_dir = temp.path().join("desktop-path");
        fs::create_dir(&path_dir).unwrap();
        let path = env::join_paths([&path_dir]).unwrap();
        let extension = temp
            .path()
            .join(".vscode/extensions/openai.chatgpt-1.0.0/bin")
            .join(if cfg!(target_arch = "aarch64") {
                if cfg!(target_os = "macos") {
                    "darwin-arm64"
                } else {
                    "linux-aarch64"
                }
            } else if cfg!(target_os = "macos") {
                "darwin-x86_64"
            } else {
                "linux-x86_64"
            })
            .join("codex");
        write_executable(&extension, b"codex");

        assert_eq!(
            resolve_command_with("codex", Some(path.as_os_str()), temp.path()),
            Some(extension)
        );
    }

    #[test]
    fn skips_non_executable_candidates_and_returns_actionable_error() {
        let temp = tempfile::tempdir().unwrap();
        let path_dir = temp.path().join("bin");
        fs::create_dir(&path_dir).unwrap();
        fs::write(path_dir.join("codex"), b"not executable").unwrap();
        let path = env::join_paths([&path_dir]).unwrap();

        let error = require_codex_command(Some(path.as_os_str()), temp.path()).unwrap_err();
        assert_eq!(
            error,
            "Codex CLI was not found. Install Codex or the OpenAI VS Code extension, then reopen Multi Codex"
        );
    }

    #[test]
    fn recognizes_both_macos_extension_architectures() {
        for platform in ["darwin-x86_64", "darwin-arm64"] {
            let temp = tempfile::tempdir().unwrap();
            let binary = temp
                .path()
                .join(".vscode/extensions/openai.chatgpt-1.0.0/bin")
                .join(platform)
                .join("codex");
            write_executable(&binary, b"codex");
            assert_eq!(
                find_extension_codex_for_platform(temp.path(), platform),
                Some(binary)
            );
        }
    }

    #[test]
    fn managed_directory_creation_rejects_symlink_escapes() {
        let (temp, service) = fixture();
        let id = Uuid::new_v4().to_string();
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        let profile_link = service.data_root.join("profiles").join(&id);
        symlink(&outside, &profile_link).unwrap();
        let result =
            ensure_private_managed_dir(&service.data_root, &profile_link.join("codex-home"));
        assert!(result.is_err());
        assert!(!outside.join("codex-home").exists());
    }

    #[test]
    fn importing_current_auth_never_changes_the_source_file() {
        let (temp, service) = fixture();
        fs::create_dir_all(&service.global_codex_home).unwrap();
        let source = service.global_codex_home.join("auth.json");
        write_private_file(&source, sample_auth().as_bytes()).unwrap();
        let before = fs::read(&source).unwrap();
        let mode_before = fs::metadata(&source).unwrap().permissions().mode() & 0o777;
        service.import_current("Current".into(), None).unwrap();
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            mode_before
        );
        assert!(temp.path().exists());
    }

    #[test]
    fn optional_notes_are_validated_and_persisted() {
        let (_temp, service) = fixture();
        let mut input = sample_input("Tracked");
        input.notes = Some("  Resets after the billing cycle  ".into());
        let profile = service.add_profile(input).unwrap();
        assert_eq!(
            profile.metadata.notes.as_deref(),
            Some("Resets after the billing cycle")
        );

        let stored = service.list_profiles().unwrap().remove(0).metadata;
        assert_eq!(stored.notes, profile.metadata.notes);
    }

    #[test]
    fn old_metadata_without_optional_fields_remains_compatible() {
        let (_temp, service) = fixture();
        let id = Uuid::new_v4().to_string();
        let metadata = format!(
            r#"[{{"id":"{id}","name":"Legacy","authMode":"ChatGPT","createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z"}}]"#
        );
        write_private_file(
            &service.data_root.join("profiles.json"),
            metadata.as_bytes(),
        )
        .unwrap();
        let profile = service.list_profiles().unwrap().remove(0);
        assert_eq!(profile.metadata.name, "Legacy");
        assert_eq!(profile.metadata.notes, None);
    }

    #[test]
    fn legacy_manual_usage_fields_are_ignored() {
        let (_temp, service) = fixture();
        let id = Uuid::new_v4().to_string();
        let metadata = format!(
            r#"[{{"id":"{id}","name":"Legacy","authMode":"ChatGPT","requestsRemaining":125,"resetDate":"2026-09-30","notes":"Keep this","createdAt":"2026-09-01T00:00:00Z","updatedAt":"2026-09-01T00:00:00Z"}}]"#
        );
        write_private_file(
            &service.data_root.join("profiles.json"),
            metadata.as_bytes(),
        )
        .unwrap();
        let profile = service.list_profiles().unwrap().remove(0);
        assert_eq!(profile.metadata.notes.as_deref(), Some("Keep this"));
    }
}
