//! Windows-native discovery, process inspection and isolated terminal launches.
use crate::profiles::Result;
use base64::{engine::general_purpose::STANDARD, Engine};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};
use sysinfo::{ProcessesToUpdate, System};

pub fn resolve(name: &str, path: Option<&OsStr>, home: &Path) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if name == "code" {
        for root in [
            std::env::var_os("LOCALAPPDATA")
                .map(PathBuf::from)
                .unwrap_or_else(|| home.join("AppData/Local")),
            std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .unwrap_or_default(),
            std::env::var_os("ProgramFiles(x86)")
                .map(PathBuf::from)
                .unwrap_or_default(),
        ] {
            candidates.extend([
                root.join("Programs/Microsoft VS Code/Code.exe"),
                root.join("Microsoft VS Code/Code.exe"),
            ]);
        }
    }
    for directory in path.map(std::env::split_paths).into_iter().flatten() {
        candidates.push(directory.join(format!("{name}.exe")));
        if name == "code" {
            candidates.push(directory.join("../Code.exe"));
        }
        // npm's codex.cmd delegates to a native executable. Use that executable
        // directly to avoid cmd.exe quoting and script-execution-policy changes.
        if name == "codex" {
            candidates.push(directory.join("node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe"));
            candidates.push(
                directory.join(
                    "node_modules/@openai/codex/vendor/x86_64-pc-windows-msvc/codex/codex.exe",
                ),
            );
        }
    }
    if name == "codex" {
        if let Some(roaming) = std::env::var_os("APPDATA") {
            let npm = PathBuf::from(roaming).join("npm");
            candidates.push(npm.join("node_modules/@openai/codex/node_modules/@openai/codex-win32-x64/vendor/x86_64-pc-windows-msvc/codex/codex.exe"));
        }
    }
    candidates
        .into_iter()
        .find(|p| crate::profiles::is_executable_file(p))
}

/// Query registered MSIX locations instead of relying on aliases or privileged
/// enumeration of Program Files/WindowsApps. Cache this slow package query.
pub fn standalone_candidates(home: &Path, path: Option<&OsStr>) -> Vec<PathBuf> {
    static PACKAGES: OnceLock<Mutex<(Vec<PathBuf>, Instant)>> = OnceLock::new();
    let mut cache = PACKAGES
        .get_or_init(|| Mutex::new((Vec::new(), Instant::now() - Duration::from_secs(60))))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if cache.1.elapsed() > Duration::from_secs(30) {
        cache.0 = crate::process::output(
            Command::new("powershell.exe").args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "Get-AppxPackage -Name OpenAI.Codex | ForEach-Object { $_.InstallLocation }",
            ]),
            Duration::from_secs(5),
        )
        .ok()
        .filter(|o| o.status.success())
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|line| PathBuf::from(line.trim()).join("app/ChatGPT.exe"))
                .collect()
        })
        .unwrap_or_default();
        cache.1 = Instant::now();
    }
    let mut candidates = cache.0.clone();
    candidates.extend(
        path.map(std::env::split_paths)
            .into_iter()
            .flatten()
            .map(|dir| dir.join("ChatGPT.exe")),
    );
    candidates.push(home.join("AppData/Local/Programs/Codex/ChatGPT.exe"));
    candidates
}

fn snapshot() -> std::sync::MutexGuard<'static, (System, Instant)> {
    static SNAPSHOT: OnceLock<Mutex<(System, Instant)>> = OnceLock::new();
    let mut guard = SNAPSHOT
        .get_or_init(|| Mutex::new((System::new(), Instant::now() - Duration::from_secs(2))))
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    if guard.1.elapsed() > Duration::from_millis(400) {
        guard.0.refresh_processes(ProcessesToUpdate::All, true);
        guard.1 = Instant::now();
    }
    guard
}

pub fn process_arguments(pid: u32) -> Option<Vec<Vec<u8>>> {
    snapshot().0.process(sysinfo::Pid::from_u32(pid)).map(|p| {
        p.cmd()
            .iter()
            .map(|a| a.to_string_lossy().as_bytes().to_vec())
            .collect()
    })
}

fn arguments_use_home(args: &[String], home: &Path) -> bool {
    args.iter().enumerate().any(|(index, arg)| {
        if Path::new(arg) == home {
            return true;
        }
        let value = arg.strip_prefix("--user-data-dir=").or_else(|| {
            (index > 0 && args[index - 1] == "--user-data-dir").then_some(arg.as_str())
        });
        value.is_some_and(|value| {
            Path::new(value) == home
                || std::fs::canonicalize(value)
                    .ok()
                    .zip(std::fs::canonicalize(home).ok())
                    .is_some_and(|(a, b)| a == b)
        })
    }) || args.windows(2).any(|pair| {
        pair[0].eq_ignore_ascii_case("-EncodedCommand") && command_uses_home(&pair[1], home)
    })
}

pub fn process_uses_home(pid: u32, home: &Path) -> bool {
    snapshot()
        .0
        .process(sysinfo::Pid::from_u32(pid))
        .is_some_and(|p| {
            let args: Vec<_> = p
                .cmd()
                .iter()
                .map(|s| s.to_string_lossy().into_owned())
                .collect();
            arguments_use_home(&args, home)
        })
}

pub fn profile_running(home: &Path) -> bool {
    snapshot().0.processes().values().any(|p| {
        let args: Vec<_> = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        arguments_use_home(&args, home)
    })
}

fn decoded_home(command: &str) -> Option<PathBuf> {
    let bytes = STANDARD.decode(command).ok()?;
    let utf16: Vec<_> = bytes
        .chunks_exact(2)
        .map(|b| u16::from_le_bytes([b[0], b[1]]))
        .collect();
    let script = String::from_utf16(&utf16).ok()?;
    let marker = script.lines().next()?.strip_prefix("# MULTI_CODEX_HOME=")?;
    Some(PathBuf::from(
        String::from_utf8(STANDARD.decode(marker).ok()?).ok()?,
    ))
}

pub(crate) fn command_uses_home(command: &str, home: &Path) -> bool {
    decoded_home(command).is_some_and(|p| p == home)
}

fn quote(value: &Path) -> String {
    format!("'{}'", value.to_string_lossy().replace('\'', "''"))
}

pub fn cli_script(codex: &Path, home: &Path, workspace: &Path) -> String {
    format!("# MULTI_CODEX_HOME={}\n$ErrorActionPreference='Stop'; Get-ChildItem Env: | Where-Object {{ $_.Name -match '^(CODEX_|OPENAI_|VSCODE_)' -or $_.Name -in @('ELECTRON_RUN_AS_NODE','NODE_OPTIONS') }} | ForEach-Object {{ Remove-Item -LiteralPath ('Env:' + $_.Name) }}; $env:CODEX_HOME={}; $env:CODEX_SQLITE_HOME={}; & {} -C {}; exit $LASTEXITCODE", STANDARD.encode(home.to_string_lossy().as_bytes()), quote(home), quote(home), quote(codex), quote(workspace))
}

pub fn launch_cli(
    terminal: crate::terminals::Terminal,
    binary: &Path,
    codex: &Path,
    home: &Path,
    workspace: &Path,
) -> Result<()> {
    use std::os::windows::process::CommandExt;
    let script = cli_script(codex, home, workspace);
    let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let mut command = Command::new(binary);
    if terminal == crate::terminals::Terminal::WindowsTerminal {
        command
            .args(["new-tab", "--startingDirectory"])
            .arg(workspace)
            .arg("powershell.exe");
    }
    command
        .args(["-NoLogo", "-NoProfile", "-EncodedCommand"])
        .arg(STANDARD.encode(bytes))
        .current_dir(workspace)
        .creation_flags(0x00000010)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("Could not open Windows terminal: {e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn powershell_command_round_trips_unicode_and_quotes_without_interpolation() {
        let home = Path::new("C:\\Users\\Test User\\工具 '$x\\codex-home");
        let script = cli_script(
            Path::new("C:\\cli\\codex.exe"),
            home,
            Path::new("C:\\project '$x"),
        );
        let bytes: Vec<_> = script.encode_utf16().flat_map(u16::to_le_bytes).collect();
        assert_eq!(decoded_home(&STANDARD.encode(bytes)).as_deref(), Some(home));
        assert!(script.contains("''$x"));
        assert!(script.contains("Remove-Item -LiteralPath"));
    }
    #[test]
    fn canonical_windows_prefixes_identify_the_same_profile_but_not_a_neighbor() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("Profile 工具 O'Brien");
        std::fs::create_dir_all(&home).unwrap();
        let canonical = std::fs::canonicalize(&home).unwrap();
        assert!(arguments_use_home(
            &[format!("--user-data-dir={}", home.display())],
            &canonical
        ));
        assert!(arguments_use_home(
            &["--user-data-dir".into(), home.display().to_string()],
            &canonical
        ));
        assert!(!arguments_use_home(
            &[format!("--user-data-dir={}-other", home.display())],
            &canonical
        ));
    }
    #[test]
    fn native_credential_store_round_trip() {
        use crate::profiles::{KeyringSecretStore, SecretStore};
        let id = uuid::Uuid::new_v4().to_string();
        let store = KeyringSecretStore;
        store.set(&id, "windows-synthetic-ci-credential").unwrap();
        assert_eq!(store.get(&id).unwrap(), "windows-synthetic-ci-credential");
        store.delete(&id).unwrap();
        assert!(store.get(&id).is_err());
    }
    #[test]
    fn private_atomic_files_replace_and_lock_on_windows() {
        let root = tempfile::tempdir().unwrap();
        let home = root.path().join("space 工具");
        std::fs::create_dir_all(&home).unwrap();
        let path = home.join("auth.json");
        crate::profiles::write_private_file(&path, b"first").unwrap();
        crate::profiles::write_private_file(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
        let lock = crate::file_security::lock_file(&home.join("lock")).unwrap();
        fs2::FileExt::lock_exclusive(&lock).unwrap();
        let peer = crate::file_security::lock_file(&home.join("lock")).unwrap();
        assert!(fs2::FileExt::try_lock_exclusive(&peer).is_err());
        drop(lock);
        assert!(fs2::FileExt::try_lock_exclusive(&peer).is_ok());
    }
}
