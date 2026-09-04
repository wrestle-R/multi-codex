use serde::Serialize;
use std::env;
use std::fs::{self, File};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;

const APP_ID: &str = "multi-codex-desktop";
const APPIMAGE_NAME: &str = "multi-codex.AppImage";
const DESKTOP_FILE_NAME: &str = "multi-codex-desktop.desktop";
const DESKTOP_SHORTCUT_NAME: &str = "Multi Codex.desktop";

const ICONS: &[(&str, &[u8])] = &[
    ("32x32", include_bytes!("../icons/32x32.png")),
    ("128x128", include_bytes!("../icons/128x128.png")),
    ("256x256", include_bytes!("../icons/128x128@2x.png")),
    ("512x512", include_bytes!("../icons/icon.png")),
];

#[derive(Clone, Debug)]
pub struct DesktopIntegration {
    source_appimage: Option<PathBuf>,
    install_bin: PathBuf,
    applications_dir: PathBuf,
    icons_dir: PathBuf,
    desktop_dir: PathBuf,
    version: String,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopIntegrationStatus {
    pub available: bool,
    pub installed: bool,
    pub desktop_shortcut: bool,
    pub version: String,
    pub source: String,
}

impl DesktopIntegration {
    pub fn discover() -> Result<Self, String> {
        let home =
            dirs::home_dir().ok_or_else(|| "Could not locate the home directory".to_string())?;
        let data_home = env::var_os("XDG_DATA_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .unwrap_or_else(|| home.join(".local/share"));
        let desktop_dir = dirs::desktop_dir().unwrap_or_else(|| home.join("Desktop"));

        Ok(Self {
            source_appimage: env::var_os("APPIMAGE")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            install_bin: home.join(".local/bin").join(APPIMAGE_NAME),
            applications_dir: data_home.join("applications"),
            icons_dir: data_home.join("icons/hicolor"),
            desktop_dir,
            version: env!("CARGO_PKG_VERSION").to_string(),
        })
    }

    pub fn status(&self) -> DesktopIntegrationStatus {
        let available = self
            .source_appimage
            .as_ref()
            .is_some_and(|path| path.is_file());
        let installed = self.install_bin.is_file()
            && self.applications_dir.join(DESKTOP_FILE_NAME).is_file()
            && ICONS.iter().all(|(size, _)| self.icon_path(size).is_file());

        DesktopIntegrationStatus {
            available,
            installed,
            desktop_shortcut: self.desktop_dir.join(DESKTOP_SHORTCUT_NAME).is_file(),
            version: self.version.clone(),
            source: if available { "appimage" } else { "package" }.to_string(),
        }
    }

    pub fn install(
        &self,
        create_desktop_shortcut: bool,
    ) -> Result<DesktopIntegrationStatus, String> {
        let source = self.source_appimage.as_ref().ok_or_else(|| {
            "Desktop integration is only needed for the portable AppImage".to_string()
        })?;
        let source = source
            .canonicalize()
            .map_err(|error| format!("Could not access the running AppImage: {error}"))?;
        if !source.is_file() {
            return Err("The running AppImage is not a regular file".to_string());
        }

        atomic_copy(&source, &self.install_bin, 0o755)?;
        for (size, bytes) in ICONS {
            atomic_write(&self.icon_path(size), bytes, 0o644)?;
        }

        let desktop_entry = desktop_entry(&self.install_bin)?;
        atomic_write(
            &self.applications_dir.join(DESKTOP_FILE_NAME),
            desktop_entry.as_bytes(),
            0o644,
        )?;
        if create_desktop_shortcut {
            atomic_write(
                &self.desktop_dir.join(DESKTOP_SHORTCUT_NAME),
                desktop_entry.as_bytes(),
                0o755,
            )?;
        }

        let _ = Command::new("update-desktop-database")
            .arg(&self.applications_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();

        Ok(self.status())
    }

    fn icon_path(&self, size: &str) -> PathBuf {
        self.icons_dir
            .join(size)
            .join("apps")
            .join(format!("{APP_ID}.png"))
    }
}

fn desktop_entry(executable: &Path) -> Result<String, String> {
    let executable = executable
        .to_str()
        .ok_or_else(|| "The AppImage install path is not valid UTF-8".to_string())?;
    let escaped = executable
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=Multi Codex\nComment=Launch isolated Codex accounts\nExec=\"{escaped}\"\nIcon={APP_ID}\nTerminal=false\nCategories=Utility;Development;\nStartupWMClass={APP_ID}\n"
    ))
}

fn prepare_parent(path: &Path) -> Result<PathBuf, String> {
    let parent = path
        .parent()
        .ok_or_else(|| "Installation target has no parent directory".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))
        .map_err(|error| format!("Could not protect {}: {error}", parent.display()))?;
    parent
        .canonicalize()
        .map_err(|error| format!("Could not resolve {}: {error}", parent.display()))
}

fn atomic_write(path: &Path, contents: &[u8], mode: u32) -> Result<(), String> {
    let parent = prepare_parent(path)?;
    let target = parent.join(
        path.file_name()
            .ok_or_else(|| "Installation target has no filename".to_string())?,
    );
    let mut temporary = NamedTempFile::new_in(&parent)
        .map_err(|error| format!("Could not create a temporary install file: {error}"))?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| format!("Could not set install permissions: {error}"))?;
    temporary
        .write_all(contents)
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not write {}: {error}", target.display()))?;
    temporary
        .persist(&target)
        .map_err(|error| format!("Could not install {}: {}", target.display(), error.error))?;
    Ok(())
}

fn atomic_copy(source: &Path, target: &Path, mode: u32) -> Result<(), String> {
    let parent = prepare_parent(target)?;
    let target = parent.join(
        target
            .file_name()
            .ok_or_else(|| "Installation target has no filename".to_string())?,
    );
    let mut input = File::open(source)
        .map_err(|error| format!("Could not open {}: {error}", source.display()))?;
    let mut temporary = NamedTempFile::new_in(&parent)
        .map_err(|error| format!("Could not create a temporary AppImage: {error}"))?;
    io::copy(&mut input, temporary.as_file_mut())
        .and_then(|_| temporary.as_file().sync_all())
        .map_err(|error| format!("Could not copy the AppImage: {error}"))?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(mode))
        .map_err(|error| format!("Could not set AppImage permissions: {error}"))?;
    temporary
        .persist(&target)
        .map_err(|error| format!("Could not install {}: {}", target.display(), error.error))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(root: &Path, with_appimage: bool) -> DesktopIntegration {
        let source = root.join("download/Multi Codex.AppImage");
        if with_appimage {
            fs::create_dir_all(source.parent().unwrap()).unwrap();
            fs::write(&source, b"portable-appimage").unwrap();
        }
        DesktopIntegration {
            source_appimage: with_appimage.then_some(source),
            install_bin: root.join("home/.local/bin").join(APPIMAGE_NAME),
            applications_dir: root.join("data/applications"),
            icons_dir: root.join("data/icons/hicolor"),
            desktop_dir: root.join("home/Desktop"),
            version: "0.2.0".to_string(),
        }
    }

    #[test]
    fn package_installation_does_not_offer_appimage_integration() {
        let root = tempfile::tempdir().unwrap();
        let status = fixture(root.path(), false).status();
        assert_eq!(status.source, "package");
        assert!(!status.available);
        assert!(!status.installed);
    }

    #[test]
    fn installs_appimage_entry_icons_and_optional_shortcut_atomically() {
        let root = tempfile::tempdir().unwrap();
        let integration = fixture(root.path(), true);
        let status = integration.install(true).unwrap();
        assert!(status.installed);
        assert!(status.desktop_shortcut);
        assert_eq!(
            fs::read(&integration.install_bin).unwrap(),
            b"portable-appimage"
        );
        assert_eq!(
            fs::metadata(&integration.install_bin)
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        assert_eq!(
            fs::metadata(integration.desktop_dir.join(DESKTOP_SHORTCUT_NAME))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
        for (size, bytes) in ICONS {
            assert_eq!(fs::read(integration.icon_path(size)).unwrap(), *bytes);
        }
        let entry =
            fs::read_to_string(integration.applications_dir.join(DESKTOP_FILE_NAME)).unwrap();
        assert!(entry.contains("Icon=multi-codex-desktop"));
        assert!(entry.contains("StartupWMClass=multi-codex-desktop"));
    }

    #[test]
    fn leaves_desktop_empty_when_shortcut_is_not_requested() {
        let root = tempfile::tempdir().unwrap();
        let integration = fixture(root.path(), true);
        let status = integration.install(false).unwrap();
        assert!(status.installed);
        assert!(!status.desktop_shortcut);
    }

    #[test]
    fn desktop_entry_escapes_exec_metacharacters() {
        let entry = desktop_entry(Path::new("/tmp/a $b/with`tick\\and\"quote.AppImage")).unwrap();
        assert!(entry.contains("\\$b"));
        assert!(entry.contains("\\`tick"));
        assert!(entry.contains("\\\\and\\\"quote"));
    }
}
