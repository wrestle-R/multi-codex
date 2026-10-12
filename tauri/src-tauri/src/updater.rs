use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::UpdaterExt;

fn updater_builder(app: &AppHandle) -> Result<tauri_plugin_updater::UpdaterBuilder, String> {
    app.updater_builder()
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
