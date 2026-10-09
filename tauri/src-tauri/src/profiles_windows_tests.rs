//! Native Windows checks use disposable homes and inert API-key fixtures.
use super::*;
use std::sync::Mutex;

#[derive(Default)]
struct MemorySecrets(Mutex<HashMap<String, String>>);
impl SecretStore for MemorySecrets {
    fn set(&self, id: &str, value: &str) -> Result<()> {
        self.0.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn get(&self, id: &str) -> Result<String> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or("missing fixture credential".into())
    }
    fn delete(&self, id: &str) -> Result<()> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}
struct AcceptAuth;
impl AuthRecognizer for AcceptAuth {
    fn recognize(&self, _: &str) -> Result<()> {
        Ok(())
    }
}

fn fixture() -> (tempfile::TempDir, ProfileService<MemorySecrets, AcceptAuth>) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp
        .path()
        .canonicalize()
        .unwrap()
        .join("Windows test 工具 O'Brien");
    let service = ProfileService::new(
        root.join("data"),
        root.join("global"),
        root.join("extensions"),
        MemorySecrets::default(),
        AcceptAuth,
    )
    .unwrap();
    fs::create_dir_all(&service.global_codex_home).unwrap();
    fs::write(
        service.global_codex_home.join("auth.json"),
        b"untouched-global-sentinel",
    )
    .unwrap();
    (temp, service)
}
fn add(service: &ProfileService<MemorySecrets, AcceptAuth>, label: &str) -> ProfileView {
    service
        .add_profile(SaveProfileInput {
            name: label.into(),
            notes: None,
            auth_json: format!(
                r#"{{"auth_mode":"apikey","OPENAI_API_KEY":"inert-windows-fixture-{label}"}}"#
            ),
        })
        .unwrap()
}

#[test]
fn windows_accounts_survive_restart_and_keep_peer_and_global_data_private() {
    let (temp, service) = fixture();
    let first = add(&service, "first");
    let second = add(&service, "second");
    let first_path = service.profile_paths(&first.metadata.id).unwrap();
    let second_path = service.profile_paths(&second.metadata.id).unwrap();
    let peer = fs::read(second_path.codex_home.join("auth.json")).unwrap();
    let data = service.data_root.clone();
    let global = service.global_codex_home.clone();
    let extensions = service.data_root.join("unused-shared-extensions");
    // Export/import uses persistent private files; simulate a fresh native process.
    let restored = ProfileService::new(
        data,
        global.clone(),
        extensions,
        MemorySecrets::default(),
        AcceptAuth,
    )
    .unwrap();
    assert_eq!(restored.list_profiles().unwrap().len(), 2);
    assert_eq!(
        restored
            .current_profile_credential(&first.metadata.id)
            .unwrap(),
        fs::read_to_string(first_path.codex_home.join("auth.json")).unwrap()
    );
    restored.delete_profile(&first.metadata.id).unwrap();
    assert_eq!(
        fs::read(second_path.codex_home.join("auth.json")).unwrap(),
        peer
    );
    assert_eq!(
        fs::read(global.join("auth.json")).unwrap(),
        b"untouched-global-sentinel"
    );
    assert!(!first_path.codex_home.exists());
    assert!(restored.profile_paths("../escape").is_err());
    drop(temp);
}

struct NativeCleanup(Vec<PathBuf>);
impl Drop for NativeCleanup {
    fn drop(&mut self) {
        // Electron can create a worker while the first process snapshot is being
        // inspected. Recheck only these disposable homes after killing a tree.
        for _ in 0..3 {
            let mut system = sysinfo::System::new_all();
            system.refresh_processes(sysinfo::ProcessesToUpdate::All, true);
            for process in system.processes().values() {
                if self.0.iter().any(|home| {
                    process_uses_profile(process.pid().as_u32(), home)
                        || process.cmd().windows(2).any(|p| {
                            p[0].to_string_lossy()
                                .eq_ignore_ascii_case("-EncodedCommand")
                                && crate::windows_platform::command_uses_home(
                                    &p[1].to_string_lossy(),
                                    home,
                                )
                        })
                }) {
                    let _ = Command::new("taskkill.exe")
                        .args(["/PID", &process.pid().as_u32().to_string(), "/T", "/F"])
                        .output();
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
}
fn wait_for(label: &str, mut condition: impl FnMut() -> bool) {
    eprintln!("Waiting for {label}");
    let end = Instant::now() + Duration::from_secs(60);
    while Instant::now() < end {
        if condition() {
            return;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    panic!("Native Windows launch did not satisfy {label}");
}
fn visible(home: &Path) -> bool {
    let system = sysinfo::System::new_all();
    system.processes().values().filter(|p| process_uses_profile(p.pid().as_u32(), home)).any(|p| {
        crate::process::output(Command::new("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", &format!("if ((Get-Process -Id {} -ErrorAction SilentlyContinue).MainWindowHandle -eq 0) {{ exit 1 }}",p.pid())]), Duration::from_secs(5)).is_ok_and(|o| o.status.success())
    })
}

#[test]
#[ignore = "requires native Windows VS Code and Codex fixture installations"]
fn windows_live_launches_keep_two_accounts_isolated_and_block_mutation() {
    let (_temp, service) = fixture();
    let first = add(&service, "first");
    let second = add(&service, "second");
    let workspace = service.data_root.join("project 工具 O'Brien");
    fs::create_dir_all(&workspace).unwrap();
    let a = service.profile_paths(&first.metadata.id).unwrap();
    let b = service.profile_paths(&second.metadata.id).unwrap();
    let originals = [
        fs::read(a.codex_home.join("auth.json")).unwrap(),
        fs::read(b.codex_home.join("auth.json")).unwrap(),
    ];
    let homes = vec![
        a.vscode_home.clone(),
        b.vscode_home.clone(),
        a.codex_home.clone(),
        b.codex_home.clone(),
        a.desktop_home.clone(),
        b.desktop_home.clone(),
    ];
    let cleanup = NativeCleanup(homes.clone());
    let code = PathBuf::from(env::var_os("MULTI_CODEX_TEST_VSCODE").expect("VS Code fixture path"));
    for profile in [&first, &second] {
        service
            .launch_profile_with_command(&profile.metadata.id, &workspace, &code)
            .unwrap();
    }
    for (profile, paths) in [(&first, &a), (&second, &b)] {
        wait_for("visible VS Code window and active account guard", || {
            visible(&paths.vscode_home) && service.is_running(&profile.metadata.id).unwrap()
        });
        assert!(service.delete_profile(&profile.metadata.id).is_err());
        assert!(service.clear_profile_cache(&profile.metadata.id).is_err());
    }
    drop(cleanup);
    for p in [&first, &second] {
        wait_for("VS Code cleanup releasing the account guard", || {
            !service.is_running(&p.metadata.id).unwrap()
        });
    }
    // Exercise the exact native PowerShell launch and marker-based CLI safety guard.
    let cli =
        PathBuf::from(env::var_os("MULTI_CODEX_TEST_ENGINE").expect("native Codex fixture path"));
    let cleanup = NativeCleanup(homes);
    service
        .launch_cli_profile_with_command(&first.metadata.id, &workspace, &cli)
        .unwrap();
    wait_for("PowerShell CLI account guard", || {
        service.is_running(&first.metadata.id).unwrap()
    });
    assert!(service.delete_profile(&first.metadata.id).is_err());
    assert!(service.clear_profile_cache(&first.metadata.id).is_err());
    drop(cleanup);
    wait_for("PowerShell CLI cleanup releasing the account guard", || {
        !service.is_running(&first.metadata.id).unwrap()
    });
    assert_eq!(
        fs::read(a.codex_home.join("auth.json")).unwrap(),
        originals[0]
    );
    assert_eq!(
        fs::read(b.codex_home.join("auth.json")).unwrap(),
        originals[1]
    );
    assert_eq!(
        fs::read(service.global_codex_home.join("auth.json")).unwrap(),
        b"untouched-global-sentinel"
    );
}

#[test]
#[ignore = "opens disposable native Codex desktop windows from the official MSIX"]
fn windows_live_standalone_launch_and_restart_preserve_two_private_homes() {
    let (_temp, service) = fixture();
    let binary = PathBuf::from(
        env::var_os("MULTI_CODEX_TEST_STANDALONE").expect("official standalone fixture"),
    );
    assert_eq!(
        crate::launch_targets::desktop_version(&binary).as_deref(),
        Some("26.930.61225"),
        "Moving official download must not expand the tested-version allowlist"
    );
    assert_eq!(
        crate::launch_targets::resolve_standalone().unwrap(),
        fs::canonicalize(&binary).unwrap(),
        "Production discovery must resolve the installed signed MSIX"
    );
    let detected = crate::launch_targets::detect();
    assert!(detected.standalone_installed && detected.standalone_verified);
    let first = add(&service, "first");
    let second = add(&service, "second");
    let a = service.profile_paths(&first.metadata.id).unwrap();
    let b = service.profile_paths(&second.metadata.id).unwrap();
    let originals = [
        fs::read(a.codex_home.join("auth.json")).unwrap(),
        fs::read(b.codex_home.join("auth.json")).unwrap(),
    ];
    let workspace = service.data_root.join("project 工具 O'Brien");
    fs::create_dir_all(&workspace).unwrap();
    for attempt in 1..=2 {
        let cleanup = NativeCleanup(vec![a.desktop_home.clone(), b.desktop_home.clone()]);
        for (p, paths) in [(&first, &a), (&second, &b)] {
            service
                .launch_standalone_profile(&p.metadata.id, &workspace)
                .unwrap();
            wait_for("visible standalone window and active account guard", || {
                visible(&paths.desktop_home) && service.is_running(&p.metadata.id).unwrap()
            });
            assert!(
                service.delete_profile(&p.metadata.id).is_err(),
                "Desktop launch {attempt} must protect account deletion"
            );
            assert!(service.clear_profile_cache(&p.metadata.id).is_err());
            assert!(paths.desktop_home.join("Local State").is_file());
        }
        assert_eq!(
            fs::read(a.codex_home.join("auth.json")).unwrap(),
            originals[0]
        );
        assert_eq!(
            fs::read(b.codex_home.join("auth.json")).unwrap(),
            originals[1]
        );
        assert_eq!(
            fs::read(service.global_codex_home.join("auth.json")).unwrap(),
            b"untouched-global-sentinel"
        );
        assert!(!a.vscode_home.exists() && !b.vscode_home.exists());
        drop(cleanup);
        wait_for("standalone cleanup releasing both account guards", || {
            !service.is_running(&first.metadata.id).unwrap()
                && !service.is_running(&second.metadata.id).unwrap()
        });
    }
}
