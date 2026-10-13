//! Native sharing/migration acceptance on each host OS, with inert private fixtures.
use super::*;
use sha2::Digest;
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
            .ok_or("missing inert credential".into())
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
fn sample_input(name: &str) -> SaveProfileInput {
    SaveProfileInput {
        name: name.into(),
        notes: None,
        auth_json: r#"{"auth_mode":"apikey","OPENAI_API_KEY":"inert-native-migration-fixture"}"#
            .into(),
    }
}
fn fixture() -> (tempfile::TempDir, ProfileService<MemorySecrets, AcceptAuth>) {
    let temp = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(temp.path()).unwrap();
    let global = root.join("global");
    fs::create_dir(&global).unwrap();
    let service = ProfileService::new(
        root.join("data"),
        global,
        root.join("unused"),
        MemorySecrets::default(),
        AcceptAuth,
    )
    .unwrap();
    (temp, service)
}

#[test]
fn verified_restore_moves_a_guarded_profile_and_keeps_current_credentials_and_work() {
    let (_temp, service) = fixture();
    let profile = service
        .add_profile(sample_input("Guarded restore fixture"))
        .unwrap();
    let id = &profile.metadata.id;
    let paths = service.profile_paths(id).unwrap();
    fs::write(
        paths.codex_home.join("original-history.fixture"),
        "original history",
    )
    .unwrap();
    let snapshot = service.prepare_profile_migration(id).unwrap();
    let latest = r#"{"auth_mode":"apikey","OPENAI_API_KEY":"inert-latest-restore-fixture"}"#;
    service
        .update_profile(id, profile.metadata.name, Some(latest.into()), None)
        .unwrap();
    fs::write(
        paths.codex_home.join("later-work.fixture"),
        "retained later work",
    )
    .unwrap();
    let restored = service.restore_profile_backup(id).unwrap();
    assert_eq!(restored.phase, "complete");
    assert_eq!(
        fs::read_to_string(paths.codex_home.join("auth.json")).unwrap(),
        latest
    );
    assert_eq!(service.secrets.get(id).unwrap(), latest);
    assert_eq!(
        fs::read_to_string(paths.codex_home.join("original-history.fixture")).unwrap(),
        "original history"
    );
    let retained = service
        .data_root
        .join("migrations/displaced-profiles")
        .join(&restored.operation_id);
    assert_eq!(
        fs::read_to_string(retained.join("codex-home/later-work.fixture")).unwrap(),
        "retained later work"
    );
    assert!(!paths.codex_home.join("later-work.fixture").exists());
    crate::migration::MigrationStore::open(&service.data_root)
        .unwrap()
        .verify(&snapshot)
        .unwrap();
}

struct OwnedRestoreEditor(PathBuf);
impl Drop for OwnedRestoreEditor {
    fn drop(&mut self) {
        // Only processes using this exact disposable profile are eligible.
        // Recheck after termination; Electron can create a child during a scan.
        for _ in 0..5 {
            let system = sysinfo::System::new_all();
            let mut found = false;
            for process in system.processes().values() {
                let pid = process.pid().as_u32();
                if !process_uses_profile(pid, &self.0) {
                    continue;
                }
                found = true;
                #[cfg(windows)]
                {
                    let _ = crate::process::background(&mut Command::new("taskkill.exe"))
                        .args(["/PID", &pid.to_string(), "/T", "/F"])
                        .output();
                }
                #[cfg(unix)]
                unsafe {
                    libc::kill(pid as i32, libc::SIGTERM);
                }
            }
            if !found {
                break;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }
}
#[test]
#[ignore = "requires explicitly supplied pinned packages and an inert native rollout"]
fn native_shared_migration_preserves_originals_imports_history_and_rolls_back() {
    let official = PathBuf::from(
        env::var_os("MULTI_CODEX_TEST_OFFICIAL_PACKAGE").expect("pinned official package"),
    );
    let bridge =
        PathBuf::from(env::var_os("MULTI_CODEX_TEST_BRIDGE_PACKAGE").expect("packaged bridge"));
    let rollout =
        PathBuf::from(env::var_os("MULTI_CODEX_TEST_ROLLOUT").expect("inert native rollout"));
    let code = env::var_os("MULTI_CODEX_TEST_VSCODE")
        .map(PathBuf::from)
        .unwrap_or_else(|| resolve_command("code").unwrap());
    let (temp, service) = fixture();
    let profile = service
        .add_profile(sample_input("Native migration fixture"))
        .unwrap();
    let id = &profile.metadata.id;
    let paths = service.profile_paths(id).unwrap();
    fs::create_dir_all(paths.codex_home.join("sessions")).unwrap();
    fs::copy(
        &rollout,
        paths
            .codex_home
            .join("sessions")
            .join(rollout.file_name().unwrap()),
    )
    .unwrap();
    fs::create_dir_all(paths.vscode_home.join("User")).unwrap();
    let settings = paths.vscode_home.join("User/settings.json");
    fs::write(&settings, "{\n // retained user preference\n \"editor.fontSize\": 17,\n \"update.mode\": \"none\",\n \"github.copilot.enable\": {\"*\": false}\n}\n").unwrap();
    fs::create_dir_all(paths.extensions_dir.join("github.copilot-1.0.0")).unwrap();
    fs::write(
        paths
            .extensions_dir
            .join("github.copilot-1.0.0/package.json"),
        r#"{"publisher":"GitHub","name":"copilot","version":"1.0.0"}"#,
    )
    .unwrap();
    fs::write(
        paths.extensions_dir.join("github.copilot-1.0.0/original"),
        "unchanged Copilot",
    )
    .unwrap();
    let probe = paths
        .extensions_dir
        .join("fixture.native-restore-probe-1.0.0");
    fs::create_dir(&probe).unwrap();
    fs::write(probe.join("package.json"), r#"{"publisher":"fixture","name":"native-restore-probe","version":"1.0.0","engines":{"vscode":"^1.95.0"},"main":"extension.cjs","activationEvents":["onStartupFinished"],"capabilities":{"untrustedWorkspaces":{"supported":true}}}"#).unwrap();
    fs::write(probe.join("extension.cjs"), r#"
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
exports.activate = () => {
  const home = process.env.CODEX_HOME;
  const value = {
    editorVersion: vscode.version,
    codexHome: home,
    sqliteHome: process.env.CODEX_SQLITE_HOME,
    workspace: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
    fontSize: vscode.workspace.getConfiguration('editor').get('fontSize'),
    copilotEnabled: vscode.workspace.getConfiguration('github.copilot').get('enable'),
    authDigest: crypto.createHash('sha256').update(fs.readFileSync(path.join(home, 'auth.json'))).digest('hex')
  };
  fs.writeFileSync(path.join(home, '.native-restored-editor-proof.json'), JSON.stringify(value), {mode: 0o600});
};
"#).unwrap();
    let auth = fs::read(paths.codex_home.join("auth.json")).unwrap();
    let legacy = crate::managed_files::tree_manifest(&paths.extensions_dir).unwrap();
    let store = crate::shared_vscode::ExtensionStore::open(&service.data_root).unwrap();
    eprintln!("native migration: publish pinned packages");
    store
        .publish(&[official, bridge.clone()], || Ok(()))
        .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::{fs::symlink, net::UnixListener};
        fs::create_dir_all(paths.codex_home.join("ipc")).unwrap();
        // Bind through a short pathname, then rename the owned stale socket into
        // the profile. Unix socket path limits otherwise reject deep fixture roots.
        let socket_parent = service
            .data_root
            .ancestors()
            .find(|path| path.as_os_str().len() < 32)
            .unwrap();
        let socket_root = tempfile::tempdir_in(socket_parent).unwrap();
        let socket = socket_root.path().join("s");
        let listener = UnixListener::bind(&socket).unwrap();
        drop(listener);
        fs::rename(&socket, paths.codex_home.join("ipc/ipc.sock")).unwrap();
        let arguments = paths.codex_home.join("tmp/arg0/codex-arg0NativeFixture");
        fs::create_dir_all(&arguments).unwrap();
        symlink(
            "/nonexistent-inert-runtime-tool",
            arguments.join("apply_patch"),
        )
        .unwrap();
    }
    #[cfg(unix)]
    {
        let old_window = paths
            .vscode_home
            .join("User/globalStorage/russeldanielpaul.multi-codex/runtime/windows")
            .join(uuid::Uuid::new_v4().to_string());
        fs::create_dir_all(&old_window).unwrap();
        fs::write(old_window.join("auth.json"), &auth).unwrap();
        std::os::unix::fs::symlink(
            paths.codex_home.join("missing-legacy-memory"),
            old_window.join("memories"),
        )
        .unwrap();
    }
    eprintln!("native migration: migrate closed inert profile");
    let record = service
        .migrate_profile_shared_storage_with_code(id, &bridge, &code)
        .unwrap();
    #[cfg(unix)]
    {
        assert!(!paths.codex_home.join("ipc/ipc.sock").exists());
        assert!(service
            .data_root
            .join("migrations/retained-runtime")
            .join(id)
            .is_dir());
        assert_eq!(fs::read(paths.codex_home.join("auth.json")).unwrap(), auth);
    }
    eprintln!("native migration: verify committed layout and preserved originals");
    assert_eq!(record.phase, crate::profile_layout::LayoutPhase::Committed);
    let catalog = crate::profile_layout::LayoutStore::open(&service.data_root)
        .unwrap()
        .selected_catalog(id)
        .unwrap()
        .unwrap();
    assert_eq!(
        fs::read_to_string(catalog.join("github.copilot-1.0.0/original")).unwrap(),
        "unchanged Copilot"
    );
    assert_eq!(
        crate::managed_files::tree_manifest(&paths.extensions_dir).unwrap(),
        legacy
    );
    assert_eq!(fs::read(paths.codex_home.join("auth.json")).unwrap(), auth);
    let library = crate::shared_chats::ChatLibrary::open(&service.data_root).unwrap();
    let saved = library.list(false, 0, 100).unwrap();
    assert_eq!(saved.len(), 1);
    let snapshot = crate::migration::MigrationStore::open(&service.data_root).unwrap();
    snapshot.verify(&record.snapshot).unwrap();
    eprintln!("native migration: compare full original and hydrated native history");
    let original = crate::shared_chat_import::read_thread(
        &service.data_root,
        &crate::managed_launch::engine(&store, &record.generation).unwrap(),
        &rollout,
    )
    .unwrap();
    let hydrated_home = service.data_root.join("native-observer");
    crate::managed_files::directory(&service.data_root, &hydrated_home).unwrap();
    let hydrated = library.hydrate(&saved[0].id, &hydrated_home).unwrap();
    let decoded = crate::shared_chat_import::read_thread(
        &service.data_root,
        &crate::managed_launch::engine(&store, &record.generation).unwrap(),
        &hydrated,
    )
    .unwrap();
    assert_eq!(decoded["turns"], original["turns"]);
    eprintln!("native migration: rollback while retaining new settings");
    let edited = fs::read_to_string(&settings).unwrap().replace("17", "21");
    fs::write(&settings, edited).unwrap();
    service.rollback_profile_shared_storage(id).unwrap();
    let restored = fs::read_to_string(&settings).unwrap();
    assert!(restored.contains("retained user preference") && restored.contains("21"));
    assert!(restored.contains("github.copilot.enable"));
    assert_eq!(library.list(false, 0, 100).unwrap().len(), 1);
    snapshot.verify(&record.snapshot).unwrap();
    eprintln!("native migration: stage verified restore");
    service.stage_profile_restore(id).unwrap();
    assert_eq!(fs::read(paths.codex_home.join("auth.json")).unwrap(), auth);
    eprintln!("native migration: activate verified restore and retain later work");
    fs::write(
        paths.codex_home.join("later-work.fixture"),
        "retained later work",
    )
    .unwrap();
    let activated = service.restore_profile_backup(id).unwrap();
    assert_eq!(activated.phase, "complete");
    let displaced = service
        .data_root
        .join("migrations/displaced-profiles")
        .join(&activated.operation_id);
    assert_eq!(
        fs::read_to_string(displaced.join("codex-home/later-work.fixture")).unwrap(),
        "retained later work"
    );
    // Restore activates the original snapshot; the displaced profile retains
    // later user settings separately. Rollback above preserves those edits in
    // place, whereas an explicitly requested full restore returns snapshot state.
    let displaced_settings =
        fs::read_to_string(displaced.join("vscode-user-data/User/settings.json")).unwrap();
    assert!(displaced_settings.contains("21"));
    assert!(displaced_settings.contains("retained user preference"));
    assert_eq!(fs::read(paths.codex_home.join("auth.json")).unwrap(), auth);
    assert_eq!(service.secrets.get(id).unwrap().as_bytes(), auth);
    assert!(paths
        .extensions_dir
        .join("fixture.native-restore-probe-1.0.0/extension.cjs")
        .is_file());
    let restored_rollout = paths
        .codex_home
        .join("sessions")
        .join(rollout.file_name().unwrap());
    let restored_thread = crate::shared_chat_import::read_thread(
        &service.data_root,
        &crate::managed_launch::engine(&store, &record.generation).unwrap(),
        &restored_rollout,
    )
    .unwrap();
    assert_eq!(restored_thread["turns"], original["turns"]);
    eprintln!("native migration: launch actual editor with restored profile/workspace");
    let workspace = temp.path().join("restored workspace with spaces");
    fs::create_dir(&workspace).unwrap();
    fs::write(workspace.join("project.fixture"), "untouched project").unwrap();
    let cleanup = OwnedRestoreEditor(paths.vscode_home.clone());
    service
        .launch_profile_with_command(id, &workspace, &code)
        .unwrap();
    let proof = paths.codex_home.join(".native-restored-editor-proof.json");
    let deadline = Instant::now() + Duration::from_secs(90);
    while !proof.exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(200));
    }
    assert!(
        proof.is_file(),
        "Restored editor extension host did not initialize"
    );
    let proof: serde_json::Value = serde_json::from_slice(&fs::read(&proof).unwrap()).unwrap();
    assert_eq!(proof["editorVersion"], "1.140.0");
    assert_eq!(
        fs::canonicalize(proof["codexHome"].as_str().unwrap()).unwrap(),
        fs::canonicalize(&paths.codex_home).unwrap()
    );
    assert_eq!(
        fs::canonicalize(proof["sqliteHome"].as_str().unwrap()).unwrap(),
        fs::canonicalize(&paths.codex_home).unwrap()
    );
    assert_eq!(
        fs::canonicalize(proof["workspace"].as_str().unwrap()).unwrap(),
        fs::canonicalize(&workspace).unwrap()
    );
    assert_eq!(proof["fontSize"], 17);
    assert_eq!(proof["copilotEnabled"]["*"], false);
    assert_eq!(
        proof["authDigest"],
        format!("{:x}", sha2::Sha256::digest(&auth))
    );
    drop(cleanup);
    assert!(
        !profile_process_running(&paths.vscode_home),
        "Owned restored editor did not stop"
    );
    snapshot.verify(&record.snapshot).unwrap();
    fn writable(path: &Path) {
        let metadata = fs::symlink_metadata(path).unwrap();
        if crate::managed_files::is_link(&metadata) {
            return;
        }
        #[cfg(unix)]
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if metadata.is_dir() { 0o700 } else { 0o600 }),
        )
        .unwrap();
        #[cfg(windows)]
        {
            let mut permissions = metadata.permissions();
            permissions.set_readonly(false);
            fs::set_permissions(path, permissions).unwrap();
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path).unwrap() {
                writable(&entry.unwrap().path());
            }
        }
    }
    writable(temp.path());
}
