use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTargets {
    pub platform: String,
    pub vscode_installed: bool,
    pub codex_cli_available: bool,
    pub standalone_installed: bool,
    pub standalone_verified: bool,
}

pub fn detect() -> LaunchTargets {
    let home = dirs::home_dir().unwrap_or_default();
    let platform = std::env::consts::OS;
    LaunchTargets {
        platform: platform.into(),
        vscode_installed: crate::profiles::resolve_command("code").is_ok(),
        codex_cli_available: crate::profiles::resolve_codex_command().is_ok(),
        standalone_installed: standalone_candidates(platform, &home, std::env::var_os("PATH"))
            .iter()
            .any(|path| crate::profiles::is_executable_file(path)),
        // Separate signed-out windows do not establish authenticated isolation.
        // Enable only after testing two identities, refresh, storage, and macOS Keychain.
        standalone_verified: false,
    }
}

fn standalone_candidates(
    platform: &str,
    home: &Path,
    path: Option<std::ffi::OsString>,
) -> Vec<PathBuf> {
    match platform {
        "macos" => [PathBuf::from("/Applications"), home.join("Applications")]
            .into_iter()
            .flat_map(|root| {
                ["Codex", "ChatGPT"]
                    .map(|name| root.join(format!("{name}.app/Contents/MacOS/{name}")))
            })
            .collect(),
        "linux" => {
            // `codex` is also the CLI name: never treat it as the desktop app.
            let mut candidates: Vec<_> = path
                .map(|paths| {
                    std::env::split_paths(&paths)
                        .map(|dir| dir.join("chatgpt"))
                        .collect()
                })
                .unwrap_or_default();
            candidates.extend([
                PathBuf::from("/usr/lib/chatgpt/ChatGPT"),
                PathBuf::from("/opt/ChatGPT/ChatGPT"),
                home.join(".local/bin/chatgpt"),
            ]);
            candidates
        }
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_detection_does_not_confuse_cli_or_other_platforms() {
        let root = tempfile::tempdir().unwrap();
        let candidates = standalone_candidates("linux", root.path(), Some(root.path().into()));
        assert!(candidates.contains(&root.path().join("chatgpt")));
        assert!(!candidates.contains(&root.path().join("codex")));
        assert!(standalone_candidates("unknown", root.path(), None).is_empty());
    }

    #[test]
    fn mac_checks_system_and_user_app_bundles_with_spaces() {
        let candidates = standalone_candidates("macos", Path::new("/Users/Test User"), None);
        assert!(candidates.contains(&PathBuf::from(
            "/Applications/Codex.app/Contents/MacOS/Codex"
        )));
        assert!(candidates.contains(&PathBuf::from(
            "/Users/Test User/Applications/ChatGPT.app/Contents/MacOS/ChatGPT"
        )));
    }
}
