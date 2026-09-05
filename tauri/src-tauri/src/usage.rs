use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::profiles::{resolve_codex_command, set_owner_only_dir, write_private_file, Result};

const FIVE_HOUR_MINS: i64 = 5 * 60;
const WEEKLY_MINS: i64 = 7 * 24 * 60;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LimitWindow {
    pub remaining_percent: u8,
    pub resets_at: Option<i64>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ProfileLimits {
    pub five_hour: Option<LimitWindow>,
    pub weekly: Option<LimitWindow>,
    pub reset_credits_available: Option<u32>,
    pub checked_at: DateTime<Utc>,
}

pub fn read_profile_limits(auth_json: &str) -> Result<ProfileLimits> {
    let codex = resolve_codex_command()?;
    read_with_command(&codex, auth_json, REQUEST_TIMEOUT, Utc::now())
}

fn read_with_command(
    codex: &Path,
    auth_json: &str,
    timeout: Duration,
    checked_at: DateTime<Utc>,
) -> Result<ProfileLimits> {
    let temp = tempfile::Builder::new()
        .prefix("multi-codex-limits-")
        .tempdir()
        .map_err(|_| "Could not create a protected limits-check directory".to_string())?;
    set_owner_only_dir(temp.path())?;
    write_private_file(&temp.path().join("auth.json"), auth_json.as_bytes())?;

    let mut child = Command::new(codex)
        .args(["app-server", "--stdio"])
        .env("CODEX_HOME", temp.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Codex CLI is required to check live limits".to_string())?;

    let result = run_protocol(&mut child, timeout, checked_at);
    let _ = child.kill();
    let _ = child.wait();
    result
}

fn run_protocol(
    child: &mut std::process::Child,
    timeout: Duration,
    checked_at: DateTime<Utc>,
) -> Result<ProfileLimits> {
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Codex limits check could not read the service response".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if sender.send(line).is_err() {
                break;
            }
        }
    });

    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| "Codex limits check could not start the service request".to_string())?;
    for message in [
        json!({
            "method": "initialize",
            "id": 1,
            "params": {
                "clientInfo": {
                    "name": "multi_codex",
                    "title": "Multi Codex",
                    "version": env!("CARGO_PKG_VERSION")
                }
            }
        }),
        json!({"method": "initialized", "params": {}}),
        json!({"method": "account/rateLimits/read", "id": 2}),
    ] {
        serde_json::to_writer(&mut *stdin, &message)
            .map_err(|_| "Codex limits check could not send the service request".to_string())?;
        stdin
            .write_all(b"\n")
            .map_err(|_| "Codex limits check could not send the service request".to_string())?;
    }
    stdin
        .flush()
        .map_err(|_| "Codex limits check could not send the service request".to_string())?;

    let deadline = Instant::now() + timeout;
    loop {
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            return Err("Codex limits check timed out".to_string());
        };
        let line = match receiver.recv_timeout(remaining) {
            Ok(Ok(line)) => line,
            Ok(Err(_)) | Err(mpsc::RecvTimeoutError::Disconnected) => {
                return Err("Codex limits service ended before returning data".to_string())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err("Codex limits check timed out".to_string())
            }
        };
        let Ok(message) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        if message.get("id").and_then(Value::as_i64) != Some(2) {
            continue;
        }
        if message.get("error").is_some() {
            return Err("Codex could not retrieve limits for this account".to_string());
        }
        return parse_limits_response(&message, checked_at);
    }
}

fn parse_limits_response(message: &Value, checked_at: DateTime<Utc>) -> Result<ProfileLimits> {
    let result = message
        .get("result")
        .and_then(Value::as_object)
        .ok_or_else(|| "Codex returned an invalid limits response".to_string())?;
    let snapshot = result
        .get("rateLimitsByLimitId")
        .and_then(Value::as_object)
        .and_then(|limits| limits.get("codex"))
        .or_else(|| result.get("rateLimits"));

    let five_hour = find_window(snapshot, FIVE_HOUR_MINS)?;
    let weekly = find_window(snapshot, WEEKLY_MINS)?;
    let reset_credits_available = match result.get("rateLimitResetCredits") {
        None | Some(Value::Null) => None,
        Some(summary) => {
            let count = summary
                .get("availableCount")
                .and_then(Value::as_u64)
                .ok_or_else(|| "Codex returned an invalid reset-credit count".to_string())?;
            Some(
                u32::try_from(count)
                    .map_err(|_| "Codex returned an invalid reset-credit count".to_string())?,
            )
        }
    };

    Ok(ProfileLimits {
        five_hour,
        weekly,
        reset_credits_available,
        checked_at,
    })
}

fn find_window(snapshot: Option<&Value>, duration_mins: i64) -> Result<Option<LimitWindow>> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    for key in ["primary", "secondary"] {
        let Some(window) = snapshot.get(key).filter(|value| !value.is_null()) else {
            continue;
        };
        if window.get("windowDurationMins").and_then(Value::as_i64) != Some(duration_mins) {
            continue;
        }
        let used = window
            .get("usedPercent")
            .and_then(Value::as_i64)
            .filter(|value| (0..=100).contains(value))
            .ok_or_else(|| "Codex returned an invalid usage percentage".to_string())?;
        let resets_at = match window.get("resetsAt") {
            None | Some(Value::Null) => None,
            Some(value) => Some(
                value
                    .as_i64()
                    .filter(|timestamp| *timestamp >= 0)
                    .ok_or_else(|| "Codex returned an invalid reset time".to_string())?,
            ),
        };
        return Ok(Some(LimitWindow {
            remaining_percent: (100 - used) as u8,
            resets_at,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    fn checked_at() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 9, 5, 4, 30, 0).unwrap()
    }

    #[test]
    fn maps_windows_by_duration_and_parses_reset_credits() {
        let response = json!({"id": 2, "result": {
            "rateLimits": {"primary": {"usedPercent": 99, "windowDurationMins": 15}},
            "rateLimitsByLimitId": {"codex": {
                "primary": {"usedPercent": 72, "windowDurationMins": 10080, "resetsAt": 1_800_000_000},
                "secondary": {"usedPercent": 18, "windowDurationMins": 300, "resetsAt": 1_700_000_000}
            }},
            "rateLimitResetCredits": {"availableCount": 3}
        }});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        assert_eq!(limits.five_hour.unwrap().remaining_percent, 82);
        assert_eq!(limits.weekly.unwrap().remaining_percent, 28);
        assert_eq!(limits.reset_credits_available, Some(3));
    }

    #[test]
    fn missing_optional_values_are_unavailable() {
        let response = json!({"id": 2, "result": {"rateLimits": {
            "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": null}
        }, "rateLimitResetCredits": null}});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        assert_eq!(limits.five_hour.unwrap().resets_at, None);
        assert_eq!(limits.weekly, None);
        assert_eq!(limits.reset_credits_available, None);
    }

    #[test]
    fn rejects_malformed_usage_without_returning_payload_data() {
        let response = json!({"id": 2, "result": {"rateLimits": {
            "primary": {"usedPercent": 101, "windowDurationMins": 300}
        }}});
        assert_eq!(
            parse_limits_response(&response, checked_at()).unwrap_err(),
            "Codex returned an invalid usage percentage"
        );
    }

    #[test]
    fn protocol_uses_private_temp_auth_and_stops_the_child() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("fake-codex");
        let pid_file = temp.path().join("pid");
        #[cfg(target_os = "linux")]
        let permission_check = "stat -c %a \"$CODEX_HOME/auth.json\"";
        #[cfg(target_os = "macos")]
        let permission_check = "stat -f %Lp \"$CODEX_HOME/auth.json\"";
        fs::write(
            &script,
            format!(
                "#!/bin/sh\nprintf '%s' $$ > '{}'\n[ \"$({permission_check})\" = 600 ] || exit 3\nread a\nread b\nread c\nprintf '%s\\n' '{{\"id\":2,\"result\":{{\"rateLimits\":{{\"primary\":{{\"usedPercent\":40,\"windowDurationMins\":300,\"resetsAt\":null}}}},\"rateLimitResetCredits\":{{\"availableCount\":1}}}}}}'\nsleep 30\n",
                pid_file.display(),
            ),
        )
        .unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let limits = read_with_command(
            &script,
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"fixture-only"}}"#,
            Duration::from_secs(2),
            checked_at(),
        )
        .unwrap();
        assert_eq!(limits.five_hour.unwrap().remaining_percent, 60);
        let pid = fs::read_to_string(pid_file).unwrap();
        assert!(!Command::new("kill")
            .args(["-0", pid.trim()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success());
    }

    #[test]
    fn protocol_timeout_is_bounded_and_sanitized() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("fake-codex");
        fs::write(&script, "#!/bin/sh\nread a\nread b\nread c\nsleep 30\n").unwrap();
        fs::set_permissions(&script, fs::Permissions::from_mode(0o700)).unwrap();
        let error = read_with_command(
            &script,
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"never-print-this"}}"#,
            Duration::from_millis(100),
            checked_at(),
        )
        .unwrap_err();
        assert_eq!(error, "Codex limits check timed out");
        assert!(!error.contains("never-print-this"));
    }
}
