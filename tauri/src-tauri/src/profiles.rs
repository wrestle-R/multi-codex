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
}

#[derive(Clone, Debug)]
struct ProfilePaths {
    codex_home: PathBuf,
    vscode_home: PathBuf,
    extensions_dir: PathBuf,
}

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
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        let mut views = Vec::with_capacity(profiles.len());
        for metadata in profiles {
            let detected = profile_process_running(&self.profile_paths(&metadata.id)?.vscode_home);
            let status = runtime
                .statuses
                .get(&metadata.id)
                .copied()
                .filter(|status| *status != RuntimeStatus::Running || detected)
                .unwrap_or(if detected {
                    RuntimeStatus::Running
                } else {
                    RuntimeStatus::Idle
                });
            let error = runtime.errors.get(&metadata.id).cloned();
            // Account plan is display-only metadata derived locally. A missing or unreadable
            // credential deliberately produces no badge rather than a guess.
            let account_tier = self
                .secrets
                .get(&metadata.id)
                .ok()
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
        })
    }

    pub(crate) fn finish_device_login(&self, pending: PendingDeviceLogin) -> Result<ProfileView> {
        let auth_json = fs::read_to_string(pending.codex_home.join("auth.json"))
            .map_err(|_| "The browser sign-in did not create a credential".to_string())?;
        let result = self.add_profile(SaveProfileInput {
            name: pending.name,
            auth_json,
            notes: pending.notes,
        });
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
        let code = resolve_command("code");
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

        let mut command = build_vscode_command_with(
            code,
            &paths.codex_home,
            &paths.vscode_home,
            &paths.extensions_dir,
            &workspace,
        );
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                self.set_error(id, format!("Could not launch VS Code: {error}"));
                return Err("Could not launch VS Code".to_string());
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
        let credential = self.current_profile_credential(id)?;
        read_profile_limits(&credential)
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
    let codex_home = home.join(".codex");
    ProfileService::new(
        data_root,
        codex_home,
        home.join(".vscode/extensions"),
        KeyringSecretStore,
        CodexCliRecognizer,
    )
}

pub fn choose_workspace() -> Result<Option<String>> {
    let home = dirs::home_dir().ok_or_else(|| "Home directory is unavailable".to_string())?;
    let desktop = home.join("Desktop");
    let initial = if desktop.is_dir() { desktop } else { home };

    #[cfg(target_os = "linux")]
    let output = Command::new("kdialog")
        .arg("--getexistingdirectory")
        .arg(&initial)
        .output()
        .or_else(|_| {
            Command::new("zenity")
                .args(["--file-selection", "--directory", "--filename"])
                .arg(format!("{}/", initial.display()))
                .output()
        })
        .map_err(|_| {
            "A desktop folder picker is required (install kdialog or zenity)".to_string()
        })?;

    #[cfg(target_os = "macos")]
    let output = Command::new("osascript")
        .args([
            "-e",
            "POSIX path of (choose folder with prompt \"Choose a workspace\" default location (path to desktop folder))",
        ])
        .output()
        .map_err(|_| "Could not open the folder picker".to_string())?;

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    return Err("Folder selection is not supported on this operating system".to_string());

    if !output.status.success() {
        return Ok(None);
    }
    let selection = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if selection.is_empty() {
        return Ok(None);
    }
    Ok(Some(
        canonical_workspace(Path::new(&selection))?
            .display()
            .to_string(),
    ))
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
        .env("CODEX_HOME", codex_home);
    command
}

pub(crate) fn resolve_command(name: &str) -> PathBuf {
    let home = dirs::home_dir().unwrap_or_default();
    resolve_command_with(name, env::var_os("PATH").as_deref(), &home)
        .unwrap_or_else(|| PathBuf::from(name))
}

pub(crate) fn resolve_codex_command() -> Result<PathBuf> {
    let home = dirs::home_dir().unwrap_or_default();
    require_codex_command(env::var_os("PATH").as_deref(), &home)
}

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

fn is_executable_file(path: &Path) -> bool {
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

#[cfg(target_os = "linux")]
fn profile_process_running(vscode_home: &Path) -> bool {
    let expected = vscode_home.as_os_str().as_encoded_bytes();
    let Ok(entries) = fs::read_dir("/proc") else {
        return false;
    };
    entries.flatten().any(|entry| {
        if !entry
            .file_name()
            .to_string_lossy()
            .chars()
            .all(|character| character.is_ascii_digit())
        {
            return false;
        }
        let Ok(cmdline) = fs::read(entry.path().join("cmdline")) else {
            return false;
        };
        cmdline
            .split(|byte| *byte == 0)
            .any(|argument| argument == expected)
    })
}

#[cfg(target_os = "macos")]
fn profile_process_running(vscode_home: &Path) -> bool {
    let Ok(output) = Command::new("ps").args(["-axo", "command="]).output() else {
        return false;
    };
    let expected = vscode_home.to_string_lossy();
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .any(|command| command.contains(expected.as_ref()))
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let workspace = temp.path().join("workspace");
        fs::create_dir(&workspace).unwrap();
        let launcher = temp.path().join("bin/code");
        write_executable(
            &launcher,
            b"#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$(dirname \"$0\")/launches\"\nsh -c 'sleep 1' multi-codex-profile \"$@\" &\n",
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
        assert!(requests
            .lines()
            .all(|request| request.contains("--new-window")));
        assert!(requests
            .lines()
            .all(|request| request.contains(workspace.to_str().unwrap())));

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
