//! Test the real updater SDK against a disposable install and loopback transport.
//! The mock runtime replaces only the GUI; download, verification and installation
//! all execute the same pinned plugin as the packaged application.
use std::{path::PathBuf, time::Duration};
use tauri_plugin_updater::UpdaterExt;

fn main() {
    assert_eq!(
        std::env::var("MULTI_CODEX_UPDATER_TEST_ALLOWED").as_deref(),
        Ok("1")
    );
    let args: Vec<String> = std::env::args().collect();
    assert_eq!(args.len(), 7);
    let endpoint = &args[1];
    let root = PathBuf::from(&args[2]).canonicalize().unwrap();
    let target_argument = PathBuf::from(&args[3]);
    let target = target_argument.canonicalize().unwrap();
    assert!(target.starts_with(&root) && target != root);
    assert!(endpoint.starts_with("http://127.0.0.1:"));
    let mode = &args[5];
    assert!(matches!(
        mode.as_str(),
        "install" | "reject" | "current" | "unavailable"
    ));
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.package_info_mut().version = "1.0.0".parse().unwrap();
    context.config_mut().plugins.0.insert(
        "updater".into(),
        serde_json::json!({
            "pubkey": args[4], "endpoints": [endpoint],
            "dangerousInsecureTransportProtocol": true,
            "windows": {"installMode": "quiet"}
        }),
    );
    let app = tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap();
    let builder = app
        .updater_builder()
        .executable_path(&target)
        .timeout(Duration::from_secs(10));
    #[cfg(windows)]
    let builder = builder
        .installer_args([format!(
            "/D={}",
            target_argument.parent().unwrap().display()
        )])
        .restart_after_install(false)
        .on_before_exit({
            let proof = PathBuf::from(&args[6]);
            let parent = proof.parent().unwrap().canonicalize().unwrap();
            assert_eq!(parent, root);
            let proof = parent.join(proof.file_name().unwrap());
            move || {
                std::fs::write(&proof, b"verified-download-before-native-installer\n").unwrap();
            }
        });
    let updater = builder.build().unwrap();
    let checked = tauri::async_runtime::block_on(updater.check());
    if mode == "unavailable" {
        assert!(checked.is_err());
        println!("SDK_CHECK_UNAVAILABLE");
        return;
    }
    if mode == "current" {
        assert!(checked.unwrap().is_none());
        println!("SDK_NO_UPDATE");
        return;
    }
    let update = checked.unwrap().expect("Expected fixture update");
    let mut downloaded = 0u64;
    let mut completed = false;
    let result = tauri::async_runtime::block_on(
        update.download_and_install(|count, _| downloaded += count as u64, || completed = true),
    );
    if mode == "reject" {
        assert!(result.is_err(), "Invalid/truncated update must be rejected");
        println!("SDK_REJECTED_UPDATE");
    } else {
        result.expect("Native SDK installation failed");
        assert!(downloaded > 0 && completed);
        println!("SDK_INSTALLED_UPDATE");
    }
}
