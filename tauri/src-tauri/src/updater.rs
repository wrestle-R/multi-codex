use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::Manager;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

static CHANNEL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    #[default]
    Stable,
    All,
}

fn read_channel(root: &std::path::Path) -> Result<UpdateChannel, String> {
    let path = root.join("update-channel.json");
    match std::fs::symlink_metadata(&path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(UpdateChannel::Stable),
        Ok(info) if info.is_file() && !info.file_type().is_symlink() && info.len() < 1024 => {
            serde_json::from_slice(
                &std::fs::read(path).map_err(|_| "Cannot read update preference")?,
            )
            .map_err(|_| "Invalid update preference".into())
        }
        _ => Err("Invalid update preference file".into()),
    }
}
#[tauri::command]
pub fn get_update_channel(app: AppHandle) -> Result<UpdateChannel, String> {
    let _lock = CHANNEL_LOCK
        .lock()
        .map_err(|_| "Update preference unavailable")?;
    read_channel(app.state::<crate::AppState>().service.data_directory())
}

#[tauri::command]
pub fn set_update_channel(channel: UpdateChannel, app: AppHandle) -> Result<(), String> {
    let state = app.state::<crate::AppState>();
    save_channel_preference(state.service.data_directory(), channel)
}
fn save_channel_preference(root: &std::path::Path, channel: UpdateChannel) -> Result<(), String> {
    let _lock = CHANNEL_LOCK
        .lock()
        .map_err(|_| "Update preference unavailable")?;
    if UPDATING.load(Ordering::Acquire) {
        return Err("Wait for the current update to finish before changing channels.".into());
    }
    write_channel(root, channel)
}
fn write_channel(root: &std::path::Path, channel: UpdateChannel) -> Result<(), String> {
    use std::io::Write;
    let path = root.join("update-channel.json");
    multi_codex_core::managed_files::checked(root, &path, true)?;
    let mut file =
        tempfile::NamedTempFile::new_in(root).map_err(|_| "Cannot save update preference")?;
    multi_codex_core::file_security::protect(file.path(), false)?;
    serde_json::to_writer(&mut file, &channel).map_err(|_| "Cannot encode update preference")?;
    file.flush()
        .and_then(|_| file.as_file().sync_all())
        .map_err(|_| "Cannot save update preference")?;
    file.persist(path)
        .map_err(|_| "Cannot save update preference")?;
    Ok(())
}

fn updater_builder(app: &AppHandle) -> Result<tauri_plugin_updater::UpdaterBuilder, String> {
    let channel = match get_update_channel(app.clone())? {
        UpdateChannel::Stable => "stable",
        UpdateChannel::All => "all",
    };
    let endpoint = url::Url::parse(&format!(
        "https://multi-codex.vercel.app/api/updates/{channel}"
    ))
    .map_err(|_| "Invalid update channel")?;
    app.updater_builder()
        .endpoints(vec![endpoint])
        .map_err(|_| "Could not select the update channel")?
        .timeout(Duration::from_secs(30))
        .header("Cache-Control", "no-cache")
        .map_err(|_| "Could not start the updater.".to_string())
}

async fn check(
    updater: &tauri_plugin_updater::Updater,
) -> Result<Option<tauri_plugin_updater::Update>, String> {
    updater.check().await.map_err(|error| {
        eprintln!("Update availability check failed: {error}");
        "Cannot reach the update service right now. Reconnect and try again.".to_string()
    })
}

#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<String>, String> {
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return Ok(None);
    }
    if UPDATING.load(Ordering::Acquire) {
        return Ok(None);
    }
    let updater = updater_builder(&app)?
        .build()
        .map_err(|_| "Could not start the updater.")?;
    Ok(check(&updater).await?.map(|update| update.version))
}

static UPDATING: AtomicBool = AtomicBool::new(false);

struct UpdateGuard;
impl Drop for UpdateGuard {
    fn drop(&mut self) {
        UPDATING.store(false, Ordering::Release);
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpdateProgress {
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
    version: Option<String>,
}

fn emit(
    app: &AppHandle,
    phase: &'static str,
    downloaded: u64,
    total: Option<u64>,
    version: Option<String>,
) {
    let _ = app.emit(
        "app-update-progress",
        UpdateProgress {
            phase,
            downloaded,
            total,
            version,
        },
    );
}

#[tauri::command]
pub async fn update_app(app: AppHandle) -> Result<String, String> {
    UPDATING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .map_err(|_| "An update is already in progress.")?;
    let _guard = UpdateGuard;
    #[cfg(target_os = "linux")]
    if std::env::var_os("APPIMAGE").is_none() {
        return Err("In-app updates require the AppImage on Linux. Update .deb and .rpm installations through their package manager.".into());
    }
    emit(&app, "checking", 0, None, None);
    let builder = updater_builder(&app)?;
    #[cfg(windows)]
    let builder = builder.on_before_exit({
        let app = app.clone();
        move || {
            emit(&app, "restarting", 0, None, None);
            std::thread::sleep(Duration::from_millis(500));
        }
    });
    let updater = builder
        .build()
        .map_err(|_| "Could not start the updater.")?;
    let update = check(&updater).await?;
    let Some(mut update) = update else {
        emit(&app, "current", 0, None, None);
        return Ok("current".into());
    };
    // Keep availability checks responsive, but allow larger packages on slower links.
    update.timeout = Some(Duration::from_secs(600));
    let version = update.version.clone();
    emit(&app, "downloading", 0, None, Some(version.clone()));
    let mut downloaded = 0;
    let mut last_event = std::time::Instant::now();
    let mut last_percent = 0;
    update.download_and_install(
        |chunk, total| {
            downloaded += chunk as u64;
            let percent = total.filter(|total| *total > 0).map(|total| (downloaded.saturating_mul(100) / total).min(100)).unwrap_or(0);
            if percent != last_percent || last_event.elapsed() >= Duration::from_millis(150) {
                emit(&app, "downloading", downloaded, total, Some(version.clone()));
                last_event = std::time::Instant::now();
                last_percent = percent;
            }
        },
        || emit(&app, "installing", 0, None, Some(version.clone())),
    ).await.map_err(|_| "The update could not be verified or installed. Your accounts and chat history are preserved. Check the install folder's permissions and try again.")?;
    emit(&app, "restarting", 0, None, Some(version));
    // Allow the webview to paint the restart state. Profile apps remain running.
    tauri::async_runtime::spawn(async move {
        // A blocking timer here runs off the async executor's worker threads.
        let _ =
            tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_millis(700)))
                .await;
        app.restart();
    });
    Ok("restarting".into())
}

#[cfg(test)]
mod channel_tests {
    use super::*;
    #[test]
    fn channel_cannot_change_during_installation_and_recovers_afterwards() {
        let root = tempfile::tempdir().unwrap();
        write_channel(root.path(), UpdateChannel::Stable).unwrap();
        assert!(!UPDATING.swap(true, Ordering::AcqRel));
        let guard = UpdateGuard;
        assert!(save_channel_preference(root.path(), UpdateChannel::All).is_err());
        assert_eq!(read_channel(root.path()).unwrap(), UpdateChannel::Stable);
        drop(guard);
        save_channel_preference(root.path(), UpdateChannel::All).unwrap();
        assert_eq!(read_channel(root.path()).unwrap(), UpdateChannel::All);
    }
    #[test]
    fn stable_default_and_all_round_trip_without_modifying_legacy_settings() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("executables.json"),
            b"{legacy-settings-canary}",
        )
        .unwrap();
        assert_eq!(read_channel(root.path()).unwrap(), UpdateChannel::Stable);
        write_channel(root.path(), UpdateChannel::All).unwrap();
        assert_eq!(read_channel(root.path()).unwrap(), UpdateChannel::All);
        write_channel(root.path(), UpdateChannel::Stable).unwrap();
        assert_eq!(read_channel(root.path()).unwrap(), UpdateChannel::Stable);
        assert_eq!(
            std::fs::read(root.path().join("executables.json")).unwrap(),
            b"{legacy-settings-canary}"
        );
        std::fs::write(root.path().join("update-channel.json"), br#""preview""#).unwrap();
        assert!(read_channel(root.path()).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn refuses_linked_preferences_and_keeps_files_private() {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("outside");
        std::fs::write(&target, b"canary").unwrap();
        let path = root.path().join("update-channel.json");
        symlink(&target, &path).unwrap();
        assert!(read_channel(root.path()).is_err());
        assert!(write_channel(root.path(), UpdateChannel::All).is_err());
        assert_eq!(std::fs::read(target).unwrap(), b"canary");
        std::fs::remove_file(&path).unwrap();
        write_channel(root.path(), UpdateChannel::All).unwrap();
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
}
