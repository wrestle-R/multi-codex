//! Prepare the bridge before Codex activation, using the installed editor runtime.
use crate::{
    managed_files as files,
    profiles::{self, Result},
    shared_vscode::{ExtensionStore, Generation},
};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub fn runtime_home(data_root: &Path, profile_id: &str) -> PathBuf {
    data_root
        .join("profiles")
        .join(profile_id)
        .join("bridge-runtime")
}

/// Resolve the CLI selected by the installed Windows editor's own launcher.
/// New builds keep resources under a version directory beside Code.exe.
#[cfg(any(windows, test))]
pub fn editor_cli_script(code: &Path) -> Result<PathBuf> {
    let root = code
        .parent()
        .ok_or("VS Code installation path is unavailable")?;
    let launcher = root.join("bin/code.cmd");
    if launcher.exists() {
        let text = fs::read_to_string(files::regular(root, &launcher)?)
            .map_err(|_| "Could not inspect the installed VS Code CLI launcher")?;
        for quoted in text.split('"') {
            let lower = quoted.to_ascii_lowercase();
            let prefix = "%~dp0..\\";
            if lower.starts_with(prefix) && lower.ends_with("\\resources\\app\\out\\cli.js") {
                let relative = quoted[prefix.len()..].replace('\\', "/");
                if !Path::new(&relative)
                    .components()
                    .all(|part| matches!(part, std::path::Component::Normal(_)))
                {
                    return Err("The installed editor CLI points outside its installation".into());
                }
                return files::regular(root, &root.join(relative));
            }
        }
    }
    files::regular(root, &root.join("resources/app/out/cli.js"))
}

pub fn editor_runtime(code: &Path) -> Result<PathBuf> {
    let resolved =
        fs::canonicalize(code).map_err(|_| "Installed VS Code executable is unavailable")?;
    #[cfg(windows)]
    let candidates = vec![resolved.clone()];
    #[cfg(target_os = "macos")]
    let candidates = if resolved.ends_with("Contents/Resources/app/bin/code") {
        let contents = resolved
            .ancestors()
            .nth(4)
            .ok_or("VS Code bundle path is unavailable")?;
        vec![contents.join("MacOS/Code"), contents.join("MacOS/Electron")]
    } else {
        vec![resolved.clone()]
    };
    #[cfg(target_os = "linux")]
    let mut candidates = if resolved.ends_with("bin/code") {
        vec![resolved
            .parent()
            .and_then(Path::parent)
            .ok_or("VS Code installation path is unavailable")?
            .join("code")]
    } else {
        vec![resolved.clone()]
    };
    #[cfg(target_os = "linux")]
    if let Ok(wrapper) = fs::read_to_string(&resolved) {
        // Follow explicit distribution CLI paths, never a different editor.
        for cli in [
            "/usr/share/code/bin/code",
            "/usr/lib/code/bin/code",
            "/opt/visual-studio-code/bin/code",
        ] {
            if wrapper.starts_with("#!") && wrapper.contains(cli) {
                candidates.push(
                    Path::new(cli)
                        .parent()
                        .and_then(Path::parent)
                        .ok_or("Installed editor path is invalid")?
                        .join("code"),
                );
            }
        }
    }
    for candidate in candidates {
        if profiles::is_executable_file(&candidate) {
            let mut header = [0u8; 2];
            use std::io::Read;
            fs::File::open(&candidate)
                .and_then(|mut file| file.read_exact(&mut header))
                .map_err(|_| "Could not verify installed VS Code runtime")?;
            if header != *b"#!" {
                // Portable data overrides both isolation flags. See the official
                // https://code.visualstudio.com/docs/setup/portable contract.
                #[cfg(not(target_os = "macos"))]
                if candidate
                    .parent()
                    .is_some_and(|root| root.join("data").exists())
                {
                    return Err("Portable VS Code overrides private account paths. Choose a standard VS Code installation; the original profile is retained".into());
                }
                #[cfg(target_os = "macos")]
                if let Some(parent) = candidate.ancestors().nth(4) {
                    if parent.join("code-portable-data").exists()
                        || parent.join("code-insiders-portable-data").exists()
                    {
                        return Err("Portable VS Code overrides private account paths. Choose a standard VS Code application; the original profile is retained".into());
                    }
                }
                return fs::canonicalize(candidate)
                    .map_err(|_| "VS Code runtime is unavailable".into());
            }
        }
    }
    Err("Choose VS Code's native executable for managed sharing; the current launcher cannot identify its JavaScript runtime".into())
}

pub fn engine(store: &ExtensionStore, generation: &Generation) -> Result<PathBuf> {
    let package = generation
        .packages
        .iter()
        .find(|package| package.id == "openai.chatgpt")
        .ok_or("Shared Codex package is missing")?;
    let root = store
        .generation_path(&generation.id)?
        .join("packages")
        .join(&package.directory);
    let cpu = match std::env::consts::ARCH {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => return Err("This CPU has not been verified for shared Codex".into()),
    };
    let platforms = match std::env::consts::OS {
        "linux" => vec![format!("linux-{cpu}")],
        "windows" if cpu == "x86_64" => vec![format!("windows-{cpu}")],
        "macos" => vec![
            format!("macos-{cpu}"),
            format!("darwin-{cpu}"),
            if cpu == "aarch64" {
                "darwin-arm64".into()
            } else {
                "darwin-x86_64".into()
            },
        ],
        _ => return Err("This platform has not been verified for shared Codex".into()),
    };
    for platform in platforms {
        let path =
            root.join("bin")
                .join(platform)
                .join(if cfg!(windows) { "codex.exe" } else { "codex" });
        if path.exists() && profiles::is_executable_file(&files::regular(&root, &path)?) {
            return Ok(path);
        }
    }
    Err("The shared Codex package has no matching native engine".into())
}

pub fn prepare(
    data_root: &Path,
    global_home: &Path,
    profile_id: &str,
    generation: &Generation,
    code: &Path,
) -> Result<BTreeMap<String, Value>> {
    if !uuid::Uuid::parse_str(profile_id).is_ok_and(|id| id.to_string() == profile_id) {
        return Err("Invalid managed profile identifier".into());
    }
    let store = ExtensionStore::open(data_root)?;
    store.verify(generation)?;
    let bridge = generation
        .packages
        .iter()
        .find(|package| package.id == "russeldanielpaul.multi-codex")
        .ok_or("Managed bridge is missing")?;
    let codex = generation
        .packages
        .iter()
        .find(|package| package.id == "openai.chatgpt")
        .ok_or("Shared Codex is missing")?;
    let packages = store.generation_path(&generation.id)?.join("packages");
    let bridge_root = packages.join(&bridge.directory);
    let helper = files::regular(
        &bridge_root,
        &bridge_root.join("bin").join(if cfg!(windows) {
            "multi-codex-account-helper.exe"
        } else {
            "multi-codex-account-helper"
        }),
    )?;
    let stamp: Value = serde_json::from_slice(
        &fs::read(files::regular(
            &bridge_root,
            &bridge_root.join("desktop-helper.json"),
        )?)
        .map_err(|_| "Managed helper metadata is missing")?,
    )
    .map_err(|_| "Managed helper metadata is invalid")?;
    if stamp["schema"] != 1
        || stamp["os"] != std::env::consts::OS
        || stamp["architecture"] != std::env::consts::ARCH
        || stamp["version"] != env!("CARGO_PKG_VERSION")
        || stamp["digest"].as_str() != Some(&files::digest(&helper)?)
    {
        return Err(
            "Managed helper does not match this app version and platform; stage a new generation"
                .into(),
        );
    }
    let engine = engine(&store, generation)?;
    let node = editor_runtime(code)?;
    let runtime = runtime_home(data_root, profile_id);
    files::directory(data_root, &runtime)?;
    let script = files::regular(&bridge_root, &bridge_root.join("dist/bridge-main.cjs"))?;
    let config = runtime.join(if cfg!(windows) {
        "codex-bridge.json"
    } else {
        "bridge-config.json"
    });
    profiles::write_private_file(&config, &serde_json::to_vec_pretty(&json!({"engine":engine,"helper":helper,"dataRoot":data_root,"globalHome":global_home,
        "startupHome":data_root.join("profiles").join(profile_id).join("codex-home"),"officialExtensionPath":packages.join(&codex.directory),"runtimeRoot":runtime,"nodeExecutable":node,"bridgeScript":script,"sharedChats":true,"profileId":profile_id})).map_err(|_| "Could not encode managed launch configuration")?)?;
    let wrapper = runtime.join(if cfg!(windows) {
        "codex-bridge.exe"
    } else {
        "codex-bridge"
    });
    files::checked(data_root, &wrapper, true)?;
    #[cfg(windows)]
    {
        let unchanged = wrapper.exists() && files::digest(&wrapper)? == files::digest(&helper)?;
        if !unchanged {
            profiles::write_private_file(
                &wrapper,
                &fs::read(&helper).map_err(|_| "Could not read managed native launcher")?,
            )?;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fn quote(path: &Path) -> Result<String> {
            Ok(format!(
                "'{}'",
                path.to_str()
                    .ok_or("Managed path is not valid Unicode")?
                    .replace('\'', "'\\''")
            ))
        }
        let launcher = format!("#!/bin/sh\nunset NODE_OPTIONS\nexport ELECTRON_RUN_AS_NODE=1\nexec {} {} --bridge-config {} \"$@\"\n", quote(&node)?, quote(&script)?, quote(&config)?);
        profiles::write_private_file(&wrapper, launcher.as_bytes())?;
        fs::set_permissions(&wrapper, fs::Permissions::from_mode(0o700))
            .map_err(|_| "Could not protect managed bridge launcher")?;
    }
    Ok(BTreeMap::from([
        ("chatgpt.openOnStartup".into(), Value::Bool(true)),
        (
            "chatgpt.cliExecutable".into(),
            Value::String(
                wrapper
                    .to_str()
                    .ok_or("Managed path is not valid Unicode")?
                    .into(),
            ),
        ),
        ("extensions.autoUpdate".into(), Value::Bool(false)),
        ("extensions.autoCheckUpdates".into(), Value::Bool(false)),
    ]))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn windows_cli_locator_supports_flat_and_versioned_resources_and_rejects_escape() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        let code = root.join("Code.exe");
        fs::create_dir_all(root.join("resources/app/out")).unwrap();
        fs::write(root.join("resources/app/out/cli.js"), "flat CLI").unwrap();
        assert_eq!(
            editor_cli_script(&code).unwrap(),
            root.join("resources/app/out/cli.js")
        );
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("07f806f999/resources/app/out")).unwrap();
        fs::write(
            root.join("07f806f999/resources/app/out/cli.js"),
            "active CLI",
        )
        .unwrap();
        fs::write(
            root.join("bin/code.cmd"),
            "\"%~dp0..\\Code.exe\" \"%~dp0..\\07f806f999\\resources\\app\\out\\cli.js\" %*\r\n",
        )
        .unwrap();
        assert_eq!(
            editor_cli_script(&code).unwrap(),
            root.join("07f806f999/resources/app/out/cli.js")
        );
        fs::write(
            root.join("bin/code.cmd"),
            "\"%~dp0..\\..\\outside\\resources\\app\\out\\cli.js\"",
        )
        .unwrap();
        assert!(editor_cli_script(&code).is_err());
    }
}
