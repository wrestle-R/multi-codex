//! Exercise the production warm-chat route against disposable Desktop profiles.
//! Any credential access is a test failure; no OS credential store is opened.
use multi_codex_core::profiles::{AuthRecognizer, ProfileService, SecretStore};
use std::path::PathBuf;

struct NoCredentials;
impl SecretStore for NoCredentials {
    fn set(&self, _: &str, _: &str) -> Result<(), String> {
        Err("Warm navigation tried to write credentials".into())
    }
    fn get(&self, _: &str) -> Result<String, String> {
        Err("Warm navigation tried to read credentials".into())
    }
    fn delete(&self, _: &str) -> Result<(), String> {
        Err("Warm navigation tried to delete credentials".into())
    }
}
impl AuthRecognizer for NoCredentials {
    fn recognize(&self, _: &str) -> Result<(), String> {
        Err("Warm navigation tried to authenticate".into())
    }
}

fn main() {
    assert_eq!(
        std::env::var("MULTI_CODEX_TEST_HUD_ROUTING").as_deref(),
        Ok("1")
    );
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 4);
    let root = PathBuf::from(&args[1]).canonicalize().unwrap();
    assert!(root.starts_with(std::env::temp_dir().canonicalize().unwrap()));
    assert!(root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with("mc-hud-desktop-"));
    assert!(root.join("disposable-hud-fixture").is_file());
    let service = ProfileService::new(
        root.join("data"),
        root.join("global"),
        root.join("extensions"),
        NoCredentials,
        NoCredentials,
    )
    .unwrap();
    service
        .open_desktop_session(&args[2], Some(&args[3]), &root)
        .unwrap();
}
