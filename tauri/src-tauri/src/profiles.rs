use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

const MAX_AUTH_BYTES: usize = 1024 * 1024;
const KEYRING_SERVICE: &str = "multi-codex";

pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveProfileInput {
    pub name: String,
    pub auth_json: String,
    #[serde(default)]
    pub requests_remaining: Option<u32>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub reset_date: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMetadata {
    pub id: String,
    pub name: String,
    pub auth_mode: String,
    #[serde(default)]
    pub requests_remaining: Option<u32>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub reset_date: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileView {
    #[serde(flatten)]
    pub metadata: ProfileMetadata,
    pub status: RuntimeStatus,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeStatus {
    Idle,
    Launching,
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

#[derive(Default)]
struct RuntimeState {
    statuses: HashMap<String, RuntimeStatus>,
    errors: HashMap<String, String>,
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

        let output = Command::new(resolve_command("codex"))
            .args(["login", "status"])
            .env("CODEX_HOME", temp.path())
            .output()
            .map_err(|error| format!("Could not run codex login status: {error}"))?;
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
    extensions_dir: PathBuf,
    secrets: Arc<S>,
    recognizer: Arc<R>,
    runtime: Arc<Mutex<RuntimeState>>,
}

impl<S: SecretStore, R: AuthRecognizer> ProfileService<S, R> {
    pub fn new(
        data_root: PathBuf,
        global_codex_home: PathBuf,
        extensions_dir: PathBuf,
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
            extensions_dir,
            secrets: Arc::new(secrets),
            recognizer: Arc::new(recognizer),
            runtime: Arc::new(Mutex::new(RuntimeState::default())),
        };
        service.clean_stale_credentials()?;
        Ok(service)
    }

    pub fn list_profiles(&self) -> Result<Vec<ProfileView>> {
        self.clean_stale_credentials()?;
        let mut profiles = self.load_metadata()?;
        profiles.sort_by_key(|profile| std::cmp::Reverse(profile.updated_at));
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        let mut views = Vec::with_capacity(profiles.len());
        for metadata in profiles {
            let detected = profile_process_running(&self.profile_paths(&metadata.id)?.1);
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
            views.push(ProfileView {
                metadata,
                status,
                error,
            });
        }
        Ok(views)
    }

    pub fn add_profile(&self, input: SaveProfileInput) -> Result<ProfileView> {
        let name = validate_name(&input.name)?;
        let (requests_remaining, notes, reset_date) =
            validate_optional_metadata(input.requests_remaining, input.notes, input.reset_date)?;
        let auth_mode = validate_auth_structure(&input.auth_json)?;
        self.recognizer.recognize(&input.auth_json)?;
        let mut profiles = self.load_metadata()?;
        ensure_unique_name(&profiles, &name, None)?;
        let now = Utc::now();
        let metadata = ProfileMetadata {
            id: Uuid::new_v4().to_string(),
            name,
            auth_mode,
            requests_remaining,
            notes,
            reset_date,
            created_at: now,
            updated_at: now,
        };
        self.secrets.set(&metadata.id, &input.auth_json)?;
        profiles.push(metadata.clone());
        if let Err(error) = self.save_metadata(&profiles) {
            let _ = self.secrets.delete(&metadata.id);
            return Err(error);
        }
        Ok(ProfileView {
            metadata,
            status: RuntimeStatus::Idle,
            error: None,
        })
    }

    pub fn import_current(
        &self,
        name: String,
        requests_remaining: Option<u32>,
        notes: Option<String>,
        reset_date: Option<String>,
    ) -> Result<ProfileView> {
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
            requests_remaining,
            notes,
            reset_date,
        })
    }

    pub fn update_profile(
        &self,
        id: &str,
        name: String,
        auth_json: Option<String>,
        requests_remaining: Option<u32>,
        notes: Option<String>,
        reset_date: Option<String>,
    ) -> Result<ProfileView> {
        validate_id(id)?;
        if self.is_running(id)? {
            return Err("Close this profile's VS Code window before editing it".to_string());
        }
        let name = validate_name(&name)?;
        let (requests_remaining, notes, reset_date) =
            validate_optional_metadata(requests_remaining, notes, reset_date)?;
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
        let auth_mode = if let Some(ref value) = auth_json {
            let mode = validate_auth_structure(value)?;
            self.recognizer.recognize(value)?;
            self.secrets.set(id, value)?;
            mode
        } else {
            profiles[index].auth_mode.clone()
        };
        profiles[index].name = name;
        profiles[index].auth_mode = auth_mode;
        profiles[index].requests_remaining = requests_remaining;
        profiles[index].notes = notes;
        profiles[index].reset_date = reset_date;
        profiles[index].updated_at = Utc::now();
        let metadata = profiles[index].clone();
        if let Err(error) = self.save_metadata(&profiles) {
            if let Some(previous) = previous_secret {
                let _ = self.secrets.set(id, &previous);
            }
            return Err(error);
        }
        Ok(ProfileView {
            metadata,
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
        let (codex_home, vscode_home) = self.profile_paths(id)?;
        debug_assert_eq!(codex_home.parent(), vscode_home.parent());
        remove_managed_tree(&self.data_root, codex_home.parent().unwrap_or(&codex_home))?;
        let mut runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        runtime.statuses.remove(id);
        runtime.errors.remove(id);
        Ok(())
    }

    pub fn launch_profile(&self, id: &str) -> Result<()> {
        validate_id(id)?;
        if !self.load_metadata()?.iter().any(|profile| profile.id == id) {
            return Err("Profile not found".to_string());
        }
        if self.is_running(id)? {
            return Err("This profile is already running".to_string());
        }

        let (codex_home, vscode_home) = self.profile_paths(id)?;
        ensure_private_managed_dir(&self.data_root, &codex_home)?;
        ensure_private_managed_dir(&self.data_root, &vscode_home)?;
        let auth_path = codex_home.join("auth.json");
        let secret = self.secrets.get(id)?;
        write_private_file(&auth_path, secret.as_bytes())?;
        drop(secret);

        {
            let mut runtime = self
                .runtime
                .lock()
                .map_err(|_| "Runtime state is unavailable")?;
            runtime
                .statuses
                .insert(id.to_string(), RuntimeStatus::Launching);
            runtime.errors.remove(id);
        }

        let mut command = build_vscode_command(&codex_home, &vscode_home, &self.extensions_dir);
        let child = match command.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = remove_private_file(&auth_path);
                self.set_error(id, format!("Could not launch VS Code: {error}"));
                return Err("Could not launch VS Code".to_string());
            }
        };

        let runtime = Arc::clone(&self.runtime);
        let profile_id = id.to_string();
        std::thread::spawn(move || {
            let mut child = child;
            {
                if let Ok(mut state) = runtime.lock() {
                    state
                        .statuses
                        .insert(profile_id.clone(), RuntimeStatus::Running);
                }
            }
            let result = child.wait();
            let cleanup = remove_private_file(&auth_path);
            if let Ok(mut state) = runtime.lock() {
                state
                    .statuses
                    .insert(profile_id.clone(), RuntimeStatus::Idle);
                if let Err(error) = result {
                    state
                        .statuses
                        .insert(profile_id.clone(), RuntimeStatus::Error);
                    state.errors.insert(
                        profile_id.clone(),
                        format!("VS Code exited unexpectedly: {error}"),
                    );
                } else if let Err(error) = cleanup {
                    state
                        .statuses
                        .insert(profile_id.clone(), RuntimeStatus::Error);
                    state.errors.insert(profile_id.clone(), error);
                } else {
                    state.errors.remove(&profile_id);
                }
            }
        });
        Ok(())
    }

    pub fn runtime_status(&self, id: &str) -> Result<ProfileRuntime> {
        validate_id(id)?;
        let (_, vscode_home) = self.profile_paths(id)?;
        let detected = profile_process_running(&vscode_home);
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

    fn load_metadata(&self) -> Result<Vec<ProfileMetadata>> {
        read_metadata(&self.data_root.join("profiles.json"))
    }

    fn save_metadata(&self, profiles: &[ProfileMetadata]) -> Result<()> {
        atomic_write_metadata(&self.data_root, profiles)
    }

    fn profile_paths(&self, id: &str) -> Result<(PathBuf, PathBuf)> {
        validate_id(id)?;
        let base = self.data_root.join("profiles").join(id);
        if base == self.global_codex_home || base.starts_with(&self.global_codex_home) {
            return Err("Refusing to use the default Codex home".to_string());
        }
        Ok((base.join("codex-home"), base.join("vscode-user-data")))
    }

    fn is_running(&self, id: &str) -> Result<bool> {
        let (_, vscode_home) = self.profile_paths(id)?;
        if profile_process_running(&vscode_home) {
            return Ok(true);
        }
        let runtime = self
            .runtime
            .lock()
            .map_err(|_| "Runtime state is unavailable")?;
        Ok(matches!(
            runtime.statuses.get(id),
            Some(RuntimeStatus::Launching | RuntimeStatus::Running)
        ))
    }

    fn clean_stale_credentials(&self) -> Result<()> {
        for profile in self.load_metadata()? {
            let (codex_home, vscode_home) = self.profile_paths(&profile.id)?;
            if !profile_process_running(&vscode_home) {
                remove_private_file(&codex_home.join("auth.json"))?;
            }
        }
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

fn validate_optional_metadata(
    requests_remaining: Option<u32>,
    notes: Option<String>,
    reset_date: Option<String>,
) -> Result<(Option<u32>, Option<String>, Option<String>)> {
    if requests_remaining.is_some_and(|value| value > 1_000_000) {
        return Err("Requests remaining must be 1,000,000 or less".to_string());
    }
    let notes = notes
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if notes
        .as_ref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err("Notes must be 500 characters or fewer".to_string());
    }
    let reset_date = reset_date
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if let Some(value) = &reset_date {
        NaiveDate::parse_from_str(value, "%Y-%m-%d")
            .map_err(|_| "Reset date must be a valid date".to_string())?;
    }
    Ok((requests_remaining, notes, reset_date))
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

fn set_owner_only_dir(path: &Path) -> Result<()> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Could not protect {}: {error}", path.display()))
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

fn write_private_file(path: &Path, bytes: &[u8]) -> Result<()> {
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

fn build_vscode_command(codex_home: &Path, vscode_home: &Path, extensions_dir: &Path) -> Command {
    let mut command = Command::new(resolve_command("code"));
    command
        .arg("--new-window")
        .arg("--wait")
        .arg("--user-data-dir")
        .arg(vscode_home)
        .arg("--extensions-dir")
        .arg(extensions_dir)
        .env("CODEX_HOME", codex_home);
    command
}

fn resolve_command(name: &str) -> PathBuf {
    if let Some(path) = env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|directory| directory.join(name))
            .find(|candidate| candidate.is_file())
    }) {
        return path;
    }

    #[cfg(target_os = "macos")]
    {
        let home = dirs::home_dir().unwrap_or_default();
        let candidates = match name {
            "code" => vec![
                PathBuf::from(
                    "/Applications/Visual Studio Code.app/Contents/Resources/app/bin/code",
                ),
                home.join("Applications/Visual Studio Code.app/Contents/Resources/app/bin/code"),
            ],
            "codex" => vec![
                home.join(".local/bin/codex"),
                PathBuf::from("/opt/homebrew/bin/codex"),
                PathBuf::from("/usr/local/bin/codex"),
            ],
            _ => Vec::new(),
        };
        if let Some(path) = candidates.into_iter().find(|candidate| candidate.is_file()) {
            return path;
        }
    }

    PathBuf::from(name)
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
            requests_remaining: None,
            notes: None,
            reset_date: None,
        }
    }

    #[test]
    fn validates_supported_shapes_without_exposing_values() {
        assert_eq!(validate_auth_structure(&sample_auth()).unwrap(), "ChatGPT");
        assert!(validate_auth_structure("[]").is_err());
        assert!(validate_auth_structure(r#"{"auth_mode":"chatgpt"}"#).is_err());
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
    fn credential_is_materialized_privately_and_cleaned_as_stale() {
        let (_temp, service) = fixture();
        let profile = service.add_profile(sample_input("Work")).unwrap();
        let (codex_home, _) = service.profile_paths(&profile.metadata.id).unwrap();
        ensure_private_managed_dir(&service.data_root, &codex_home).unwrap();
        let auth_path = codex_home.join("auth.json");
        write_private_file(&auth_path, sample_auth().as_bytes()).unwrap();
        assert_eq!(
            fs::metadata(&auth_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        service.clean_stale_credentials().unwrap();
        assert!(!auth_path.exists());
    }

    #[test]
    fn delete_removes_only_selected_profile_data_and_secret() {
        let (_temp, service) = fixture();
        let first = service.add_profile(sample_input("First")).unwrap();
        let second = service.add_profile(sample_input("Second")).unwrap();
        let (first_home, _) = service.profile_paths(&first.metadata.id).unwrap();
        let (second_home, _) = service.profile_paths(&second.metadata.id).unwrap();
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
        let command = build_vscode_command(&codex, &vscode, &extensions);
        let args: Vec<_> = command.get_args().map(|value| value.to_owned()).collect();
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--user-data-dir" && pair[1] == vscode.as_os_str()));
        assert!(args
            .windows(2)
            .any(|pair| pair[0] == "--extensions-dir" && pair[1] == extensions.as_os_str()));
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
        service
            .import_current("Current".into(), None, None, None)
            .unwrap();
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(
            fs::metadata(&source).unwrap().permissions().mode() & 0o777,
            mode_before
        );
        assert!(temp.path().exists());
    }

    #[test]
    fn optional_profile_metadata_is_validated_and_persisted() {
        let (_temp, service) = fixture();
        let mut input = sample_input("Tracked");
        input.requests_remaining = Some(125);
        input.notes = Some("  Resets after the billing cycle  ".into());
        input.reset_date = Some("2026-09-30".into());
        let profile = service.add_profile(input).unwrap();
        assert_eq!(profile.metadata.requests_remaining, Some(125));
        assert_eq!(
            profile.metadata.notes.as_deref(),
            Some("Resets after the billing cycle")
        );
        assert_eq!(profile.metadata.reset_date.as_deref(), Some("2026-09-30"));

        let stored = service.list_profiles().unwrap().remove(0).metadata;
        assert_eq!(stored.requests_remaining, Some(125));
        assert_eq!(stored.notes, profile.metadata.notes);
        assert_eq!(stored.reset_date, profile.metadata.reset_date);

        let mut invalid = sample_input("Invalid");
        invalid.reset_date = Some("2026-02-30".into());
        assert!(service.add_profile(invalid).is_err());
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
        assert_eq!(profile.metadata.requests_remaining, None);
        assert_eq!(profile.metadata.notes, None);
        assert_eq!(profile.metadata.reset_date, None);
    }
}
