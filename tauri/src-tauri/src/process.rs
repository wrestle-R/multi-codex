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
