//! Runner-only diagnosis of native Keychain access with one inert entry.
fn main() {
    assert_eq!(std::env::var("GITHUB_ACTIONS").as_deref(), Ok("true"));
    let id = format!("native-keychain-fixture-{}", uuid::Uuid::new_v4());
    let result = (|| -> keyring::Result<()> {
        let entry = keyring::Entry::new("multi-codex", &id)?;
        entry.set_password("inert-keychain-diagnostic")?;
        let read = entry.get_password();
        let cleanup = entry.delete_credential();
        assert_eq!(read?, "inert-keychain-diagnostic");
        cleanup
    })();
    let report = match &result {
        Ok(()) => serde_json::json!({"passed": true}),
        Err(error) => serde_json::json!({"passed": false, "nativeError": error.to_string()}),
    };
    println!("{report}");
    if result.is_err() {
        std::process::exit(1);
    }
}
