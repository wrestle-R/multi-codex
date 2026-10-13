use crate::{
    app::Service,
    error::{CliError, Result},
};
use multi_codex_core::profiles::{resolve_codex_command, PendingDeviceLogin, ProfileView};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};

pub fn sign_in(
    service: &Service,
    pending: PendingDeviceLogin,
    no_browser: bool,
    stop: &Arc<AtomicBool>,
) -> Result<ProfileView> {
    let result = run(&pending, no_browser, stop);
    match result {
        Ok(()) => service.finish_device_login(pending).map_err(Into::into),
        Err(error) => {
            service.abandon_device_login(pending)?;
            Err(error)
        }
    }
}
fn run(pending: &PendingDeviceLogin, no_browser: bool, stop: &Arc<AtomicBool>) -> Result<()> {
    let engine = resolve_codex_command()?;
    let mut command = Command::new(engine);
    for (key, _) in std::env::vars_os() {
        let name = key.to_string_lossy();
        if name.starts_with("CODEX_") || name.starts_with("OPENAI_") || name == "NODE_OPTIONS" {
            command.env_remove(key);
        }
    }
    let mut child = command
        .args(["login", "--device-auth"])
        .env("CODEX_HOME", &pending.codex_home)
        .env("CODEX_SQLITE_HOME", &pending.codex_home)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| CliError::new(3, "Could not start the official Codex engine."))?;
    let (send, receive) = mpsc::channel();
    for source in [
        child
            .stdout
            .take()
            .map(|r| Box::new(r) as Box<dyn std::io::Read + Send>),
        child
            .stderr
            .take()
            .map(|r| Box::new(r) as Box<dyn std::io::Read + Send>),
    ]
    .into_iter()
    .flatten()
    {
        let send = send.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(source)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if let Some(code) = device_code(&line) {
                    let _ = send.send(code);
                }
            }
        });
    }
    drop(send);
    let deadline = Instant::now() + Duration::from_secs(15 * 60);
    let mut shown = false;
    loop {
        if stop.load(Ordering::SeqCst) || Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(CliError::new(
                if stop.load(Ordering::SeqCst) { 130 } else { 5 },
                "Sign-in cancelled or expired; temporary state removed.",
            ));
        }
        if let Ok(code) = receive.try_recv() {
            if !shown {
                shown = true;
                eprintln!("Sign in at https://auth.openai.com/codex/device\nDevice code: {code}");
                if !no_browser {
                    let mut browser = if cfg!(target_os = "macos") {
                        Command::new("/usr/bin/open")
                    } else {
                        Command::new("xdg-open")
                    };
                    let _ = browser
                        .arg("https://auth.openai.com/codex/device")
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .spawn();
                }
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                return if status.success() {
                    Ok(())
                } else {
                    Err(CliError::new(
                        5,
                        "Codex sign-in did not complete. Retry accounts add or reconnect.",
                    ))
                }
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(CliError::new(5, "Could not observe Codex sign-in."));
            }
        }
    }
}
fn device_code(line: &str) -> Option<String> {
    line.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .find(|part| {
            part.len() == 9
                && part.as_bytes()[4] == b'-'
                && part
                    .bytes()
                    .enumerate()
                    .all(|(i, c)| i == 4 || c.is_ascii_uppercase() || c.is_ascii_digit())
        })
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    #[test]
    fn instructions_filter_does_not_echo_credential_diagnostics() {
        assert_eq!(
            super::device_code("Use ABCD-1234"),
            Some("ABCD-1234".into())
        );
        assert_eq!(super::device_code("access_token=private-value"), None);
        assert_eq!(super::device_code("lower-abcd"), None);
    }
}
