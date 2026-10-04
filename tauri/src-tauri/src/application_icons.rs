//! Read installed application names and icons without invoking desktop files.
use crate::desktop_environment::DesktopInventory;
use base64::Engine;
use gio::prelude::*;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

#[derive(Clone)]
struct Application {
    name: String,
    icon: Option<String>,
}
struct Cache {
    updated: Instant,
    applications: HashMap<String, Application>,
    resolved: HashMap<String, Application>,
}
static CACHE: OnceLock<Mutex<Option<Cache>>> = OnceLock::new();

pub fn decorate(snapshot: &mut DesktopInventory) {
    let Ok(mut cache) = CACHE.get_or_init(|| Mutex::new(None)).lock() else {
        return;
    };
    if cache
        .as_ref()
        .is_none_or(|cache| cache.updated.elapsed() > Duration::from_secs(60))
    {
        *cache = Some(Cache {
            updated: Instant::now(),
            applications: installed_applications(),
            resolved: HashMap::new(),
        });
    }
    let Some(cache) = cache.as_mut() else {
        return;
    };
    for window in snapshot
        .desktops
        .iter_mut()
        .flat_map(|desktop| &mut desktop.windows)
    {
        let key = window.application.to_ascii_lowercase();
        let application = cache.resolved.entry(key.clone()).or_insert_with(|| {
            let mut application = cache
                .applications
                .get(&key)
                .cloned()
                .unwrap_or(Application {
                    name: window.application.clone(),
                    icon: None,
                });
            if key == "multi-codex-desktop" {
                application.name = "Multi Codex".into();
                application.icon = Some(format!(
                    "data:image/png;base64,{}",
                    base64::engine::general_purpose::STANDARD
                        .encode(include_bytes!("../icons/32x32.png"))
                ));
            } else if matches!(
                key.as_str(),
                "com.microsoft.vscode" | "code" | "visual studio code"
            ) {
                application.name = "Visual Studio Code".into();
                if application.icon.is_none() {
                    application.icon = fs::read_link(format!("/proc/{}/exe", window.pid))
                        .ok()
                        .and_then(|exe| {
                            icon_data(&exe.parent()?.join("resources/app/resources/linux/code.png"))
                        });
                }
            }
            application
        });
        window.application = application.name.clone();
        window.icon = application.icon.clone();
    }
}

fn installed_applications() -> HashMap<String, Application> {
    let mut result = HashMap::new();
    let icon_roots = icon_roots();
    for app in gio::AppInfo::all() {
        let Some(desktop) = app.downcast_ref::<gio::DesktopAppInfo>() else {
            continue;
        };
        let icon = desktop.icon().and_then(|icon| {
            if let Some(file) = icon.downcast_ref::<gio::FileIcon>() {
                return file.file().path().and_then(|path| icon_data(&path));
            }
            let themed = icon.downcast_ref::<gio::ThemedIcon>()?;
            themed
                .names()
                .iter()
                .find_map(|name| find_icon(name, &icon_roots))
        });
        let application = Application {
            name: desktop.display_name().to_string(),
            icon,
        };
        for key in application_keys(desktop) {
            result
                .entry(key.to_ascii_lowercase())
                .or_insert_with(|| application.clone());
        }
    }
    result
}

fn application_keys(desktop: &gio::DesktopAppInfo) -> Vec<String> {
    let mut keys = vec![desktop.name().to_string()];
    if let Some(id) = desktop.id() {
        keys.push(id.trim_end_matches(".desktop").into());
    }
    if let Some(class) = desktop.startup_wm_class() {
        keys.push(class.to_string());
    }
    // D-Bus activated entries need not have Exec. GIO's executable() binding
    // assumes a non-null value, so identify applications without calling it.
    keys
}

fn icon_roots() -> Vec<PathBuf> {
    let mut bases = vec![];
    if let Some(home) = dirs::home_dir() {
        bases.push(home.join(".icons"));
    }
    if let Some(data) = dirs::data_dir() {
        bases.push(data.join("icons"));
    }
    for base in std::env::split_paths(
        &std::env::var_os("XDG_DATA_DIRS")
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "/usr/local/share:/usr/share".into()),
    ) {
        if base.is_absolute() {
            bases.push(base.join("icons"));
            bases.push(base.join("pixmaps"));
        }
    }
    let mut roots = vec![];
    for base in bases {
        roots.push(base.clone());
        roots.push(base.join("hicolor"));
        if let Ok(themes) = fs::read_dir(base) {
            let mut themes: Vec<_> = themes
                .flatten()
                .filter(|theme| theme.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|theme| theme.path())
                .collect();
            themes.sort();
            roots.extend(themes);
        }
    }
    roots
}

fn find_icon(name: &str, roots: &[PathBuf]) -> Option<String> {
    let path = Path::new(name);
    if path.is_absolute() {
        return icon_data(path);
    }
    if path.components().count() != 1 || name == "." || name == ".." {
        return None;
    }
    for root in roots {
        for folder in [
            "128x128/apps",
            "64x64/apps",
            "48x48/apps",
            "scalable/apps",
            "32x32/apps",
            "256x256/apps",
            "symbolic/apps",
            "",
        ] {
            for extension in ["png", "svg"] {
                let file = if path.extension().is_some() {
                    root.join(folder).join(name)
                } else {
                    root.join(folder).join(format!("{name}.{extension}"))
                };
                if let Some(data) = icon_data(&file) {
                    return Some(data);
                }
            }
        }
    }
    None
}

fn icon_data(path: &Path) -> Option<String> {
    let metadata = fs::metadata(path).ok()?;
    if !metadata.is_file() || metadata.len() > 256 * 1024 {
        return None;
    }
    let extension = path.extension()?.to_str()?;
    let bytes = fs::read(path).ok()?;
    let mime = match extension {
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => "image/png",
        "svg" if std::str::from_utf8(&bytes).ok()?.contains("<svg") => "image/svg+xml",
        _ => return None,
    };
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_dbus_activated_entries_without_executables() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("org.multicodex.IconTest.desktop");
        fs::write(
            &path,
            "[Desktop Entry]\nType=Application\nName=D-Bus Example\nDBusActivatable=true\n",
        )
        .unwrap();
        let desktop = gio::DesktopAppInfo::from_filename(path).unwrap();
        assert!(application_keys(&desktop).contains(&"D-Bus Example".to_string()));
    }
    #[test]
    fn reads_installed_images_but_rejects_non_images_and_oversized_files() {
        let root = tempfile::tempdir().unwrap();
        let png = root.path().join("icon.png");
        fs::write(&png, include_bytes!("../icons/32x32.png")).unwrap();
        assert!(icon_data(&png)
            .unwrap()
            .starts_with("data:image/png;base64,"));
        fs::write(root.path().join("secret.json"), "private").unwrap();
        assert!(icon_data(&root.path().join("secret.json")).is_none());
        fs::write(&png, vec![0; 256 * 1024 + 1]).unwrap();
        assert!(icon_data(&png).is_none());
        assert!(find_icon("../secret.json", &[root.path().to_path_buf()]).is_none());
    }
}
