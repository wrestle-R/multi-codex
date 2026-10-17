//! Native sharing/migration acceptance on each host OS, with inert private fixtures.
use super::*;
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
    fs::write(&settings, "{\n // retained user preference\n \"editor.fontSize\": 17,\n \"github.copilot.enable\": {\"*\": false}\n}\n").unwrap();
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
    let auth = fs::read(paths.codex_home.join("auth.json")).unwrap();
    let legacy = crate::managed_files::tree_manifest(&paths.extensions_dir).unwrap();
    let store = crate::shared_vscode::ExtensionStore::open(&service.data_root).unwrap();
    store
        .publish(&[official, bridge.clone()], || Ok(()))
        .unwrap();
    let record = service
        .migrate_profile_shared_storage_with_code(id, &bridge, &code)
        .unwrap();
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
    let edited = fs::read_to_string(&settings).unwrap().replace("17", "21");
    fs::write(&settings, edited).unwrap();
    service.rollback_profile_shared_storage(id).unwrap();
    let restored = fs::read_to_string(&settings).unwrap();
    assert!(restored.contains("retained user preference") && restored.contains("21"));
    assert!(restored.contains("github.copilot.enable"));
    assert_eq!(library.list(false, 0, 100).unwrap().len(), 1);
    snapshot.verify(&record.snapshot).unwrap();
    service.stage_profile_restore(id).unwrap();
    assert_eq!(fs::read(paths.codex_home.join("auth.json")).unwrap(), auth);
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
