//! T3 desktop launches use owned backend and Electron directories, never a global instance.
use crate::{
    managed_files,
    profiles::{self, Result},
};
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub fn candidates(platform: &str, home: &Path, path: Option<std::ffi::OsString>) -> Vec<PathBuf> {
    let names: &[&str] = match platform {
        "linux" => &["t3code", "t3-code", "t3code-desktop"],
        "windows" => &[
            "T3 Code.exe",
            "T3 Code (Beta).exe",
            "T3 Code (Alpha).exe",
            "T3 Code (Nightly).exe",
            "t3code.exe",
        ],
        _ => return vec![],
    };
    let mut result: Vec<_> = path
        .map(|p| {
            env::split_paths(&p)
                .flat_map(|dir| names.iter().map(move |n| dir.join(n)))
                .collect()
        })
        .unwrap_or_default();
    if platform == "linux" {
        result.extend([
            home.join(".local/bin/t3code"),
            PathBuf::from("/opt/t3code-bin/t3code"),
            PathBuf::from("/opt/T3 Code/t3code"),
            PathBuf::from("/opt/t3code/t3code"),
        ]);
    } else {
        for base in [
            home.join("AppData/Local/Programs"),
            home.join("AppData/Local"),
            PathBuf::from("C:/Program Files"),
        ] {
            for app in [
                "t3code",
                "T3 Code",
                "T3 Code (Beta)",
                "T3 Code (Alpha)",
                "T3 Code (Nightly)",
                "t3code-desktop",
            ] {
                for name in names {
                    result.push(base.join(app).join(name));
                }
            }
        }
    }
    result
}

pub fn resolve() -> Result<PathBuf> {
    if cfg!(target_os = "macos") {
        return Err(
            "T3 Code account launching is currently available on Linux and Windows.".into(),
        );
    }
    if let Some(path) = env::var_os("MULTI_CODEX_T3_PATH") {
        let path = PathBuf::from(path);
        if path.is_absolute() && profiles::is_executable_file(&path) {
            return Ok(path);
        }
        return Err("MULTI_CODEX_T3_PATH must be an absolute T3 desktop executable path.".into());
    }
    candidates(env::consts::OS, &dirs::home_dir().unwrap_or_default(), env::var_os("PATH"))
        .into_iter().find(|p| profiles::is_executable_file(p))
        .ok_or_else(|| "T3 Code desktop was not found. Install it or set MULTI_CODEX_T3_PATH, then reopen Multi Codex.".into())
}

pub fn prepare(root: &Path, home: &Path, codex_home: &Path, codex: &Path) -> Result<()> {
    for dir in [
        home.to_path_buf(),
        home.join("userdata"),
        home.join("config"),
        home.join("data"),
        home.join("cache"),
        home.join("local"),
    ] {
        managed_files::directory(root, &dir)?;
    }
    let settings = home.join("userdata/settings.json");
    managed_files::checked(root, &settings, true)?;
    let config = serde_json::json!({"enabled":true,"setupMode":"existing","binaryPath":codex,"homePath":codex_home,"shadowHomePath":"","launchArgs":""});
    let mut value = if settings.exists() {
        let meta = fs::metadata(&settings).map_err(|_| "Could not inspect T3 settings")?;
        if meta.len() > 1024 * 1024 {
            return Err("T3 settings are too large".into());
        }
        serde_json::from_slice::<serde_json::Value>(
            &fs::read(&settings).map_err(|_| "Could not read T3 settings")?,
        )
        .map_err(|_| "T3 settings are invalid; repair them before launching")?
    } else {
        serde_json::json!({})
    };
    let object = value
        .as_object_mut()
        .ok_or("T3 settings must be an object")?;
    let instances = object
        .entry("providerInstances")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or("T3 provider settings are invalid")?;
    instances.insert("codex".into(), serde_json::json!({"driver":"codex","displayName":"Multi Codex account","enabled":true,"config":config}));
    // Older T3 builds use providers rather than providerInstances.
    let providers = object
        .entry("providers")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .ok_or("T3 legacy provider settings are invalid")?;
    providers.insert("codex".into(), config);
    profiles::write_private_file(
        &settings,
        &serde_json::to_vec_pretty(&value).map_err(|_| "Could not encode T3 settings")?,
    )
}

pub fn command(binary: &Path, home: &Path, codex_home: &Path, workspace: &Path) -> Command {
    let mut command = Command::new(binary);
    for (key, _) in env::vars_os() {
        let name = key.to_string_lossy();
        if ["CODEX_", "OPENAI_", "VSCODE_", "T3CODE_"]
            .iter()
            .any(|p| name.starts_with(p))
        {
            command.env_remove(key);
        }
    }
    command
        .env_remove("NODE_OPTIONS")
        .env_remove("ELECTRON_RUN_AS_NODE")
        .env_remove("APPIMAGE")
        .env_remove("APPDIR")
        .env_remove("ARGV0")
        .env_remove("OWD")
        .env_remove("LD_LIBRARY_PATH")
        .env_remove("LD_PRELOAD")
        .env("CODEX_HOME", codex_home)
        .env("CODEX_SQLITE_HOME", codex_home)
        .env("T3CODE_HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("XDG_CACHE_HOME", home.join("cache"));
    #[cfg(windows)]
    command
        .env("APPDATA", home.join("config"))
        .env("LOCALAPPDATA", home.join("local"));
    command
        .arg(format!(
            "--user-data-dir={}",
            home.join("config/t3code-v2").display()
        ))
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn macos_keeps_t3_disabled_while_the_launcher_remains_available() {
        assert!(resolve().unwrap_err().contains("Linux and Windows"));
        assert!(!crate::launch_targets::detect().t3_installed);
    }

    #[test]
    fn t3_desktop_candidates_never_choose_the_t3_server_cli() {
        for platform in ["linux", "windows"] {
            let paths = candidates(platform, Path::new("/fixture"), Some("/fixture/bin".into()));
            assert!(!paths.iter().any(|p| p.file_name().unwrap() == "t3"));
            assert!(!paths.is_empty());
        }
        assert!(candidates("macos", Path::new("/fixture"), None).is_empty());
    }
    #[test]
    fn t3_two_accounts_keep_provider_credentials_and_electron_state_separate() {
        let root = tempfile::tempdir().unwrap();
        for id in ["one", "two"] {
            let home = root.path().join(id);
            let codex = root.path().join(format!("codex-{id}"));
            prepare(root.path(), &home, &codex, Path::new("/fixture/codex")).unwrap();
            let file = home.join("userdata/settings.json");
            let mut value: serde_json::Value =
                serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
            assert_eq!(
                value["providerInstances"]["codex"]["config"]["homePath"],
                codex.to_str().unwrap()
            );
            assert_eq!(
                value["providerInstances"]["codex"]["config"]["setupMode"],
                "existing"
            );
            value["unrelated"] = serde_json::json!({"keep":true});
            fs::write(&file, serde_json::to_vec(&value).unwrap()).unwrap();
            prepare(root.path(), &home, &codex, Path::new("/fixture/codex")).unwrap();
            let saved: serde_json::Value =
                serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            assert_eq!(saved["unrelated"]["keep"], true);
            let command = command(Path::new("t3code"), &home, &codex, root.path());
            let env: std::collections::HashMap<_, _> = command.get_envs().collect();
            assert_eq!(
                env[std::ffi::OsStr::new("T3CODE_HOME")],
                Some(home.as_os_str())
            );
            assert_eq!(
                env[std::ffi::OsStr::new("CODEX_HOME")],
                Some(codex.as_os_str())
            );
            #[cfg(windows)]
            assert_eq!(
                env[std::ffi::OsStr::new("APPDATA")],
                Some(home.join("config").as_os_str())
            );
            assert!(command
                .get_args()
                .next()
                .unwrap()
                .to_string_lossy()
                .contains(id));
        }
    }
    #[cfg(unix)]
    #[test]
    fn t3_settings_refuse_symlink_escapes() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::os::unix::fs::symlink(outside.path(), root.path().join("t3")).unwrap();
        assert!(prepare(
            root.path(),
            &root.path().join("t3"),
            Path::new("/auth"),
            Path::new("/codex")
        )
        .is_err());
        assert!(!outside.path().join("userdata").exists());
    }
}
