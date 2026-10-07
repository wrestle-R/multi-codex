use crate::profiles::{resolve_command, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Terminal {
    #[default]
    Automatic,
    Terminal,
    Kitty,
    Ghostty,
    Konsole,
    GnomeTerminal,
    XfceTerminal,
    Alacritty,
    Foot,
    Xterm,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOption {
    id: Terminal,
    label: &'static str,
    available: bool,
}

impl Terminal {
    fn label(self) -> &'static str {
        match self {
            Self::Automatic => "Automatic",
            Self::Terminal => "Terminal",
            Self::Kitty => "Kitty",
            Self::Ghostty => "Ghostty",
            Self::Konsole => "Konsole",
            Self::GnomeTerminal => "GNOME Terminal",
            Self::XfceTerminal => "XFCE Terminal",
            Self::Alacritty => "Alacritty",
            Self::Foot => "Foot",
            Self::Xterm => "xterm",
        }
    }

    fn command(self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Ghostty => "ghostty",
            Self::Konsole => "konsole",
            Self::GnomeTerminal => "gnome-terminal",
            Self::XfceTerminal => "xfce4-terminal",
            Self::Alacritty => "alacritty",
            Self::Foot => "foot",
            Self::Xterm => "xterm",
            _ => "",
        }
    }

    fn prefix(self) -> &'static [&'static str] {
        match self {
            Self::GnomeTerminal => &["--"],
            Self::XfceTerminal => &["-x"],
            Self::Ghostty | Self::Konsole | Self::Alacritty | Self::Xterm => &["-e"],
            _ => &[],
        }
    }
}

fn candidates() -> &'static [Terminal] {
    #[cfg(target_os = "linux")]
    return &[
        Terminal::Konsole,
        Terminal::GnomeTerminal,
        Terminal::XfceTerminal,
        Terminal::Xterm,
        Terminal::Kitty,
        Terminal::Ghostty,
        Terminal::Alacritty,
        Terminal::Foot,
    ];
    #[cfg(target_os = "macos")]
    return &[
        Terminal::Terminal,
        Terminal::Kitty,
        Terminal::Ghostty,
        Terminal::Alacritty,
    ];
}

fn binary(terminal: Terminal) -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        if terminal == Terminal::Terminal {
            return Some(PathBuf::from("/usr/bin/open"));
        }
        let app = match terminal {
            Terminal::Kitty => "kitty.app",
            Terminal::Ghostty => "Ghostty.app",
            Terminal::Alacritty => "Alacritty.app",
            _ => return None,
        };
        for root in [
            PathBuf::from("/Applications"),
            dirs::home_dir()?.join("Applications"),
        ] {
            let path = root
                .join(app)
                .join("Contents/MacOS")
                .join(terminal.command());
            if crate::profiles::is_executable_file(&path) {
                return Some(path);
            }
        }
        if terminal == Terminal::Ghostty {
            return None;
        }
    }
    if terminal.command().is_empty() {
        None
    } else {
        resolve_command(terminal.command()).ok()
    }
}

pub fn options() -> Vec<TerminalOption> {
    std::iter::once(TerminalOption {
        id: Terminal::Automatic,
        label: "Automatic",
        available: candidates()
            .iter()
            .any(|terminal| binary(*terminal).is_some()),
    })
    .chain(candidates().iter().map(|terminal| TerminalOption {
        id: *terminal,
        label: terminal.label(),
        available: binary(*terminal).is_some(),
    }))
    .collect()
}

fn choose_with(
    preference: Terminal,
    supported: &[Terminal],
    resolve: impl Fn(Terminal) -> Option<PathBuf>,
) -> Result<(Terminal, PathBuf)> {
    if preference != Terminal::Automatic {
        return supported.contains(&preference)
            .then(|| resolve(preference)).flatten()
            .map(|path| (preference, path))
            .ok_or_else(|| format!("{} is unavailable. Install it or choose another CLI terminal in Launch settings.", preference.label()));
    }
    supported.iter().find_map(|terminal| resolve(*terminal).map(|path| (*terminal, path)))
        .ok_or_else(|| "No supported terminal is installed. Choose a CLI terminal in Launch settings after installing one.".into())
}

pub fn choose(preference: Terminal) -> Result<(Terminal, PathBuf)> {
    choose_with(preference, candidates(), binary)
}

pub fn launch(terminal: Terminal, binary: &Path, args: &[String], workspace: &Path) -> Result<()> {
    let mut command = Command::new(binary);
    #[cfg(target_os = "macos")]
    if terminal == Terminal::Ghostty {
        // Ghostty's macOS executable exposes helper actions; launch a new GUI
        // instance through Launch Services so the initial command is honored.
        let app = binary
            .parent()
            .and_then(Path::parent)
            .and_then(Path::parent)
            .filter(|path| path.extension().is_some_and(|extension| extension == "app"))
            .ok_or_else(|| {
                "Ghostty.app was not found. Install the macOS app in Applications.".to_string()
            })?;
        command = Command::new("/usr/bin/open");
        command.args(["-n", "-a"]).arg(app).arg("--args");
    }
    command
        .args(terminal.prefix())
        .args(args)
        .current_dir(workspace)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("Could not open {}: {error}", terminal.label()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_honors_preference_and_never_silently_falls_back() {
        let supported = [Terminal::Konsole, Terminal::Kitty, Terminal::Ghostty];
        let resolve = |terminal: Terminal| {
            (terminal != Terminal::Ghostty).then(|| PathBuf::from(terminal.command()))
        };
        assert_eq!(
            choose_with(Terminal::Automatic, &supported, resolve)
                .unwrap()
                .0,
            Terminal::Konsole
        );
        assert_eq!(
            choose_with(Terminal::Kitty, &supported, resolve).unwrap().0,
            Terminal::Kitty
        );
        assert!(choose_with(Terminal::Ghostty, &supported, resolve)
            .unwrap_err()
            .contains("Ghostty is unavailable"));
        assert!(choose_with(Terminal::Terminal, &supported, resolve).is_err());
        assert!(choose_with(Terminal::Automatic, &supported, |_| None).is_err());
    }

    #[test]
    fn native_terminal_arguments_keep_isolated_command_as_separate_arguments() {
        let root = tempfile::tempdir().unwrap();
        let capture = root.path().join("capture");
        let fake = root.path().join("fake terminal");
        std::fs::write(
            &fake,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n",
                capture.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&fake, std::fs::Permissions::from_mode(0o700)).unwrap();
        let args = vec![
            "/usr/bin/env".into(),
            "CODEX_HOME=/private/home with 'quotes'".into(),
            "/cli path/codex".into(),
            "-C".into(),
            "/work space".into(),
        ];
        for terminal in [
            Terminal::Kitty,
            Terminal::Konsole,
            Terminal::GnomeTerminal,
            Terminal::XfceTerminal,
            Terminal::Alacritty,
            Terminal::Foot,
            Terminal::Xterm,
        ] {
            let _ = std::fs::remove_file(&capture);
            launch(terminal, &fake, &args, root.path()).unwrap();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            let actual = loop {
                if let Ok(value) = std::fs::read_to_string(&capture) {
                    if value.ends_with("/work space\n") {
                        break value;
                    }
                }
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(10));
            };
            let expected = terminal
                .prefix()
                .iter()
                .map(|arg| arg.to_string())
                .chain(args.clone())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n";
            assert_eq!(actual, expected);
        }
    }
}
