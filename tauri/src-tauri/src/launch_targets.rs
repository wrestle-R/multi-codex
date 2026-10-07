use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchTargets {
    pub platform: String,
    pub vscode_installed: bool,
    pub codex_cli_available: bool,
    pub standalone_installed: bool,
    pub standalone_verified: bool,
    #[serde(default)]
    pub standalone_version: Option<String>,
}

pub fn detect() -> LaunchTargets {
    let home = dirs::home_dir().unwrap_or_default();
    let platform = std::env::consts::OS;
    let desktop = find_standalone(platform, &home, std::env::var_os("PATH"));
    let version = desktop.as_deref().and_then(desktop_version);
    LaunchTargets {
        platform: platform.into(),
        vscode_installed: crate::profiles::resolve_command("code").is_ok(),
        codex_cli_available: crate::profiles::resolve_interactive_codex_command().is_ok(),
        standalone_installed: desktop.is_some(),
        standalone_verified: verified_version(platform, std::env::consts::ARCH, version.as_deref()),
        standalone_version: version,
    }
}

fn verified_version(platform: &str, architecture: &str, version: Option<&str>) -> bool {
    // This internal desktop isolation API was exercised with two real identities,
    // cold credential reuse, refresh and logout. Unknown versions fail closed.
    matches!(
        (platform, architecture),
        ("linux", "x86_64") | ("macos", "aarch64")
    ) && version == Some("26.930.51102")
}

fn find_standalone(
    platform: &str,
    home: &Path,
    path: Option<std::ffi::OsString>,
) -> Option<PathBuf> {
    standalone_candidates(platform, home, path)
        .into_iter()
        .find(|path| crate::profiles::is_executable_file(path))
        .and_then(|path| std::fs::canonicalize(path).ok())
}

pub(crate) fn resolve_standalone() -> crate::profiles::Result<PathBuf> {
    let binary = find_standalone(
        std::env::consts::OS,
        &dirs::home_dir().unwrap_or_default(),
        std::env::var_os("PATH"),
    )
    .ok_or("Codex desktop app was not found. Install it and reopen Launch settings.")?;
    if !verified_version(
        std::env::consts::OS,
        std::env::consts::ARCH,
        desktop_version(&binary).as_deref(),
    ) {
        return Err("This Codex desktop version has not passed account-isolation verification. Use VS Code until it is verified.".into());
    }
    Ok(binary)
}

pub(crate) fn bundled_standalone_cli() -> Option<PathBuf> {
    let binary = find_standalone(
        std::env::consts::OS,
        &dirs::home_dir().unwrap_or_default(),
        std::env::var_os("PATH"),
    )?;
    let resources = if std::env::consts::OS == "macos" {
        binary.parent()?.parent()?.join("Resources")
    } else {
        binary.parent()?.join("resources")
    };
    let cli = if std::env::consts::OS == "macos" {
        resources.join("codex-cli/CodexCLI.app/Contents/MacOS/codex")
    } else {
        resources.join("codex")
    };
    crate::profiles::is_executable_file(&cli).then_some(cli)
}

fn desktop_version(binary: &Path) -> Option<String> {
    let parent = binary.parent()?;
    for archive in [
        parent.join("resources/app.asar"),
        parent.parent()?.join("Resources/app.asar"),
    ] {
        let Some(value) = (|| -> Option<String> {
            let mut source = File::open(archive).ok()?;
            let mut header = [0u8; 16];
            source.read_exact(&mut header).ok()?;
            let header_size = u32::from_le_bytes(header[12..16].try_into().ok()?) as usize;
            if header_size > 32 * 1024 * 1024 {
                return None;
            }
            let base = 8u64 + u32::from_le_bytes(header[4..8].try_into().ok()?) as u64;
            let mut bytes = vec![0; header_size];
            source.read_exact(&mut bytes).ok()?;
            let tree: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            let package = tree.get("files")?.get("package.json")?;
            if package.get("unpacked").and_then(|v| v.as_bool()) == Some(true) {
                return None;
            }
            let size = package.get("size")?.as_u64()?;
            if size > 65536 {
                return None;
            }
            let offset = package.get("offset")?.as_str()?.parse::<u64>().ok()?;
            source
                .seek(SeekFrom::Start(base.checked_add(offset)?))
                .ok()?;
            let mut bytes = vec![0; size as usize];
            source.read_exact(&mut bytes).ok()?;
            let package: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
            Some(package.get("version")?.as_str()?.to_string())
        })() else {
            continue;
        };
        return Some(value);
    }
    None
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

    #[test]
    fn unknown_versions_and_unverified_architectures_cannot_launch() {
        assert!(verified_version("linux", "x86_64", Some("26.930.51102")));
        assert!(verified_version("macos", "aarch64", Some("26.930.51102")));
        assert!(!verified_version("linux", "x86_64", Some("26.931.1")));
        assert!(!verified_version("macos", "x86_64", Some("26.930.51102")));
        assert!(!verified_version("linux", "x86_64", None));
    }
}
