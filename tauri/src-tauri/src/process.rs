//! Bound external desktop helpers so a disconnected compositor cannot hang a launch.
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::{Duration, Instant};

pub fn background(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

pub fn spawn(command: &mut Command) -> std::io::Result<std::process::Child> {
    background(command);
    #[cfg(target_os = "linux")]
    let deadline = Instant::now() + Duration::from_millis(250);
    loop {
        match command.spawn() {
            // A concurrent fork can briefly retain a recently closed writable
            // executable handle until exec closes it. Retry Linux ETXTBSY only;
            // a failed spawn has not started the requested process.
            #[cfg(target_os = "linux")]
            Err(error) if error.raw_os_error() == Some(26) && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(10));
            }
            result => return result,
        }
    }
}

pub fn output(command: &mut Command, timeout: Duration) -> Result<Output, String> {
    background(command);
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start platform helper: {e}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or("Platform helper stdout unavailable")?;
    let mut stderr = child
        .stderr
        .take()
        .ok_or("Platform helper stderr unavailable")?;
    let out = thread::spawn(move || {
        let mut b = Vec::new();
        stdout.read_to_end(&mut b).map(|_| b)
    });
    let err = thread::spawn(move || {
        let mut b = Vec::new();
        stderr.read_to_end(&mut b).map(|_| b)
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Platform helper timed out or became unavailable".into());
            }
        }
    };
    Ok(Output {
        status,
        stdout: out
            .join()
            .map_err(|_| "Platform helper reader failed")?
            .map_err(|_| "Could not read platform helper output")?,
        stderr: err
            .join()
            .map_err(|_| "Platform helper reader failed")?
            .map_err(|_| "Could not read platform helper errors")?,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn retries_a_busy_executable_and_bounds_a_persistent_write_lock() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("fixture");
        let mut writer = std::fs::File::create(&path).unwrap();
        writer.write_all(b"#!/bin/sh\nexit 0\n").unwrap();
        writer.sync_all().unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        let release = thread::spawn(move || {
            thread::sleep(Duration::from_millis(50));
            drop(writer);
        });
        assert!(spawn(&mut Command::new(&path))
            .unwrap()
            .wait()
            .unwrap()
            .success());
        release.join().unwrap();

        let _writer = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        let started = Instant::now();
        assert_eq!(
            spawn(&mut Command::new(&path)).unwrap_err().raw_os_error(),
            Some(26)
        );
        assert!(started.elapsed() < Duration::from_secs(2));
    }
}
