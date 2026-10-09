use crate::profiles::Result;
use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(all(test, unix))]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LaunchMode {
    #[default]
    Vscode,
    Standalone,
    Both,
    Cli,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutableSettings {
    pub code_path: Option<String>,
    pub codex_path: Option<String>,
    #[serde(default)]
    pub global_codex_home: Option<String>,
    #[serde(default)]
    pub preferred_workspace: Option<String>,
    #[serde(default)]
    pub hide_desktop_picker: bool,
    #[serde(default)]
    pub onboarding_completed: bool,
    #[serde(default)]
    pub detected_apps: Option<crate::launch_targets::LaunchTargets>,
    #[serde(default)]
    pub launch_mode: LaunchMode,
    #[serde(default)]
    pub cli_terminal: crate::terminals::Terminal,
}

pub fn data_root() -> Result<PathBuf> {
    #[cfg(windows)]
    if let Some(path) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
        let path = PathBuf::from(path);
        if !path.is_absolute() {
            return Err("APPDATA must be an absolute path".into());
        }
        return Ok(path.join("multi-codex"));
    }
    dirs::data_dir()
        .map(|p| p.join("multi-codex"))
        .ok_or_else(|| "App data directory is unavailable".into())
}

pub fn load() -> Result<ExecutableSettings> {
    load_from(&data_root()?)
}
fn load_from(root: &Path) -> Result<ExecutableSettings> {
    let path = root.join("executables.json");
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ExecutableSettings::default())
        }
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
            let bytes =
                fs::read(path).map_err(|e| format!("Could not read executable settings: {e}"))?;
            serde_json::from_slice(&bytes).map_err(|_| "Executable settings are invalid".into())
        }
        _ => Err("Executable settings must be a regular file".into()),
    }
}

pub fn save(mut settings: ExecutableSettings) -> Result<ExecutableSettings> {
    for value in [
        &mut settings.code_path,
        &mut settings.codex_path,
        &mut settings.global_codex_home,
        &mut settings.preferred_workspace,
    ] {
        if value.as_deref().is_some_and(|v| v.trim().is_empty()) {
            *value = None;
        }
    }
    for (name, value) in [
        ("VS Code", &settings.code_path),
        ("Codex", &settings.codex_path),
    ] {
        if let Some(value) = value {
            let path = Path::new(value);
            if !path.is_absolute() || !crate::profiles::is_executable_file(path) {
                return Err(format!(
                    "{name} path must point to an executable file using an absolute path"
                ));
            }
        }
    }
    if let Some(home) = &settings.global_codex_home {
        if !Path::new(home).is_absolute() || !Path::new(home).is_dir() {
            return Err(
                "Global Codex home must be an existing directory using an absolute path".into(),
            );
        }
    }
    if let Some(workspace) = &settings.preferred_workspace {
        if !Path::new(workspace).is_absolute() || !Path::new(workspace).is_dir() {
            return Err(
                "Preferred folder must be an existing directory using an absolute path".into(),
            );
        }
    }
    save_to(&data_root()?, &settings)?;
    Ok(settings)
}

fn save_to(root: &Path, settings: &ExecutableSettings) -> Result<()> {
    crate::profiles::write_private_file(
        &root.join("executables.json"),
        &serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
}

pub fn global_codex_home(
    home: &Path,
    environment: Option<std::ffi::OsString>,
    settings: &ExecutableSettings,
) -> Result<PathBuf> {
    let path = environment
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| settings.global_codex_home.as_ref().map(PathBuf::from))
        .unwrap_or_else(|| home.join(".codex"));
    if !path.is_absolute() {
        return Err("CODEX_HOME must be an absolute path".into());
    }
    Ok(path)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn saves_setup_snapshot_without_changing_existing_profile_data() {
        let root = tempfile::tempdir().unwrap();
        let profile_dir = root.path().join("profiles/test/codex-home");
        fs::create_dir_all(&profile_dir).unwrap();
        fs::write(profile_dir.join("auth.json"), b"test-only-credential").unwrap();
        fs::write(root.path().join("profiles.json"), b"test-only-metadata").unwrap();
        let settings = ExecutableSettings {
            onboarding_completed: true,
            detected_apps: Some(crate::launch_targets::LaunchTargets {
                platform: "macos".into(),
                vscode_installed: true,
                codex_cli_available: false,
                standalone_installed: true,
                standalone_verified: false,
                standalone_version: None,
            }),
            ..Default::default()
        };
        save_to(root.path(), &settings).unwrap();
        let saved = load_from(root.path()).unwrap();
        assert!(saved.onboarding_completed);
        assert_eq!(saved.detected_apps.unwrap().platform, "macos");
        assert_eq!(
            fs::read(profile_dir.join("auth.json")).unwrap(),
            b"test-only-credential"
        );
        assert_eq!(
            fs::read(root.path().join("profiles.json")).unwrap(),
            b"test-only-metadata"
        );
    }

    #[test]
    fn old_settings_keep_desktop_picker_enabled_and_new_preferences_persist() {
        let root = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("executables.json"),
            br#"{"codePath":null,"codexPath":null,"globalCodexHome":null}"#,
        )
        .unwrap();
        let mut settings = load_from(root.path()).unwrap();
        assert_eq!(settings.launch_mode, LaunchMode::Vscode);
        assert!(!settings.hide_desktop_picker);
        assert!(settings.preferred_workspace.is_none());
        settings.hide_desktop_picker = true;
        settings.preferred_workspace = Some("/Users/test/My Projects".into());
        settings.launch_mode = LaunchMode::Both;
        settings.cli_terminal = crate::terminals::Terminal::Kitty;
        save_to(root.path(), &settings).unwrap();
        let saved = load_from(root.path()).unwrap();
        assert!(saved.hide_desktop_picker);
        assert_eq!(saved.preferred_workspace, settings.preferred_workspace);
        assert_eq!(saved.launch_mode, LaunchMode::Both);
        assert_eq!(saved.cli_terminal, crate::terminals::Terminal::Kitty);
    }

    #[test]
    fn round_trips_paths_with_spaces_without_relocating_profiles() {
        let root = tempfile::tempdir().unwrap();
        let settings = ExecutableSettings {
            code_path: Some("/Applications/Visual Studio Code.app/bin/code".into()),
            codex_path: Some("/home/user/工具/codex".into()),
            global_codex_home: None,
            ..Default::default()
        };
        save_to(root.path(), &settings).unwrap();
        assert_eq!(
            load_from(root.path()).unwrap().code_path,
            settings.code_path
        );
        assert_eq!(
            fs::metadata(root.path().join("executables.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    #[test]
    fn explicit_codex_home_wins_and_relative_environment_is_rejected() {
        let settings = ExecutableSettings {
            global_codex_home: Some("/saved/codex".into()),
            ..Default::default()
        };
        assert_eq!(
            global_codex_home(
                Path::new("/home/user"),
                Some("/custom/codex".into()),
                &settings
            )
            .unwrap(),
            Path::new("/custom/codex")
        );
        assert!(
            global_codex_home(Path::new("/home/user"), Some("relative".into()), &settings).is_err()
        );
        assert_eq!(
            global_codex_home(Path::new("/home/user"), None, &settings).unwrap(),
            Path::new("/saved/codex")
        );
    }
}
