use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crate::profiles::{resolve_codex_command, Result};

const FIVE_HOUR_MINS: i64 = 5 * 60;
const WEEKLY_MINS: i64 = 7 * 24 * 60;
const MONTHLY_MINS: i64 = 30 * 24 * 60;
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
    pub monthly: Option<LimitWindow>,
    pub reset_credits_available: Option<u32>,
    pub reset_credits: Option<Vec<ResetCredit>>,
    pub checked_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ResetCredit {
    pub id: String,
    pub status: String,
    pub reset_type: String,
    pub granted_at: i64,
    pub expires_at: Option<i64>,
    pub title: Option<String>,
    pub description: Option<String>,
}

pub fn read_profile_limits(codex_home: &Path) -> Result<ProfileLimits> {
    let codex = resolve_codex_command()?;
    read_with_command(&codex, codex_home, REQUEST_TIMEOUT, Utc::now())
}

fn read_with_command(
    codex: &Path,
    codex_home: &Path,
    timeout: Duration,
    checked_at: DateTime<Utc>,
) -> Result<ProfileLimits> {
    let mut child = Command::new(codex)
        .args(["app-server", "--stdio"])
        // Use the profile's durable home so a token refreshed by Codex is not discarded with a
        // temporary directory. Each Multi Codex profile already has its own isolated CODEX_HOME.
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| "Codex CLI is required to check live limits".to_string())?;

    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Codex limits check could not read diagnostics".to_string())?;
    let (diagnostic_sender, diagnostic_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut diagnostic = String::new();
        let _ = BufReader::new(stderr).read_to_string(&mut diagnostic);
        let _ = diagnostic_sender.send(diagnostic);
    });
    let result = run_protocol(&mut child, timeout, checked_at);
    let _ = child.kill();
    let _ = child.wait();
    let diagnostic = diagnostic_receiver
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_default();
    result.map_err(|error| classify_limit_failure(error, &diagnostic))
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
        if let Some(error) = message.get("error") {
            return Err(protocol_error_message(error));
        }
        return parse_limits_response(&message, checked_at);
    }
}

fn protocol_error_message(error: &Value) -> String {
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if message.contains("unauthorized")
        || message.contains("refresh token")
        || message.contains("sign in")
    {
        return expired_session_message();
    }
    "Codex did not provide limits for this account".to_string()
}

fn classify_limit_failure(error: String, diagnostic: &str) -> String {
    let diagnostic = diagnostic.to_ascii_lowercase();
    if diagnostic.contains("refresh token")
        && (diagnostic.contains("401") || diagnostic.contains("unauthorized"))
    {
        return expired_session_message();
    }
    if diagnostic.contains("network")
        || diagnostic.contains("connection")
        || diagnostic.contains("dns")
        || diagnostic.contains("timed out")
    {
        return "Codex could not reach the limits service. Check your connection and try again"
            .to_string();
    }
    error
}

fn expired_session_message() -> String {
    "This saved profile is safe, but its Codex session expired. Open Edit and choose Sign in again before checking limits"
        .to_string()
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
    let monthly = find_window(snapshot, MONTHLY_MINS)?;
    let (reset_credits_available, reset_credits) = match result.get("rateLimitResetCredits") {
        None | Some(Value::Null) => (None, None),
        Some(summary) => {
            let count = summary
                .get("availableCount")
                .and_then(Value::as_u64)
                .ok_or_else(|| "Codex returned an invalid reset-credit count".to_string())?;
            let available = Some(
                u32::try_from(count)
                    .map_err(|_| "Codex returned an invalid reset-credit count".to_string())?,
            );
            let credits = match summary.get("credits") {
                None | Some(Value::Null) => None,
                Some(Value::Array(credits)) => Some(
                    credits
                        .iter()
                        .map(parse_reset_credit)
                        .collect::<Result<Vec<_>>>()?,
                ),
                Some(_) => return Err("Codex returned invalid reset-credit details".to_string()),
            };
            (available, credits)
        }
    };

    Ok(ProfileLimits {
        five_hour,
        weekly,
        monthly,
        reset_credits_available,
        reset_credits,
        checked_at,
    })
}

fn parse_reset_credit(value: &Value) -> Result<ResetCredit> {
    let field = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "Codex returned invalid reset-credit details".to_string())
    };
    let granted_at = value
        .get("grantedAt")
        .and_then(Value::as_i64)
        .filter(|timestamp| *timestamp >= 0)
        .ok_or_else(|| "Codex returned invalid reset-credit details".to_string())?;
    let expires_at = match value.get("expiresAt") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_i64()
                .filter(|timestamp| *timestamp >= 0)
                .ok_or_else(|| "Codex returned invalid reset-credit details".to_string())?,
        ),
    };
    let optional_string = |key: &str| match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => value
            .as_str()
            .map(|value| Some(value.to_string()))
            .ok_or_else(|| "Codex returned invalid reset-credit details".to_string()),
    };
    Ok(ResetCredit {
        id: field("id")?,
        status: field("status")?,
        reset_type: field("resetType")?,
        granted_at,
        expires_at,
        title: optional_string("title")?,
        description: optional_string("description")?,
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

    fn write_executable_script(path: &Path, contents: &str) {
        let mut file = fs::File::create(path).unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.sync_all().unwrap();
        drop(file);
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
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
        assert_eq!(limits.monthly, None);
        assert_eq!(limits.reset_credits_available, Some(3));
        assert_eq!(limits.reset_credits, None);
    }

    #[test]
    fn parses_reset_credit_expiry_details_when_the_backend_provides_them() {
        let response = json!({"id": 2, "result": {
            "rateLimits": null,
            "rateLimitResetCredits": {
                "availableCount": 1,
                "credits": [{
                    "id": "credit-1",
                    "status": "available",
                    "resetType": "codexRateLimits",
                    "grantedAt": 1_700_000_000,
                    "expiresAt": 1_800_000_000,
                    "title": "Usage reset",
                    "description": "Restores the current usage window"
                }]
            }
        }});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        let credit = &limits.reset_credits.unwrap()[0];
        assert_eq!(credit.id, "credit-1");
        assert_eq!(credit.expires_at, Some(1_800_000_000));
        assert_eq!(credit.title.as_deref(), Some("Usage reset"));
    }

    #[test]
    fn missing_optional_values_are_unavailable() {
        let response = json!({"id": 2, "result": {"rateLimits": {
            "primary": {"usedPercent": 25, "windowDurationMins": 300, "resetsAt": null}
        }, "rateLimitResetCredits": null}});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        assert_eq!(limits.five_hour.unwrap().resets_at, None);
        assert_eq!(limits.weekly, None);
        assert_eq!(limits.monthly, None);
        assert_eq!(limits.reset_credits_available, None);
    }

    #[test]
    fn parses_the_monthly_window_reported_for_free_accounts() {
        let response = json!({"id": 2, "result": {
            "rateLimitsByLimitId": {"codex": {
                "primary": {"usedPercent": 63, "windowDurationMins": 43200, "resetsAt": 1_792_841_898},
                "secondary": null
            }},
            "rateLimitResetCredits": {"availableCount": 0}
        }});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        assert_eq!(limits.five_hour, None);
        assert_eq!(limits.weekly, None);
        assert_eq!(limits.monthly.unwrap().remaining_percent, 37);
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
    fn classifies_revoked_sessions_without_exposing_diagnostics() {
        assert_eq!(
            classify_limit_failure(
                "Codex limits service ended before returning data".into(),
                "ERROR Failed to refresh token: 401 Unauthorized: sensitive server response",
            ),
            "This saved profile is safe, but its Codex session expired. Open Edit and choose Sign in again before checking limits"
        );
    }

    #[test]
    fn keeps_missing_limit_windows_as_a_valid_response() {
        let response = json!({"id": 2, "result": {"rateLimits": null, "rateLimitResetCredits": {"availableCount": 0}}});
        let limits = parse_limits_response(&response, checked_at()).unwrap();
        assert_eq!(limits.five_hour, None);
        assert_eq!(limits.weekly, None);
        assert_eq!(limits.reset_credits_available, Some(0));
    }

    #[test]
    fn protocol_preserves_refreshed_auth_and_stops_the_child() {
        let temp = tempfile::tempdir().unwrap();
        let script = temp.path().join("fake-codex");
        let pid_file = temp.path().join("pid");
        let codex_home = temp.path().join("codex-home");
        fs::create_dir(&codex_home).unwrap();
        fs::write(codex_home.join("auth.json"), "original").unwrap();
        #[cfg(target_os = "linux")]
        let permission_check = "stat -c %a \"$CODEX_HOME/auth.json\"";
        #[cfg(target_os = "macos")]
        let permission_check = "stat -f %Lp \"$CODEX_HOME/auth.json\"";
        write_executable_script(
            &script,
            &format!(
                "#!/bin/sh\nprintf '%s' $$ > '{}'\n[ \"$({permission_check})\" = 600 ] || exit 3\nread a\nread b\nread c\nprintf refreshed > \"$CODEX_HOME/auth.json\"\nprintf '%s\\n' '{{\"id\":2,\"result\":{{\"rateLimits\":{{\"primary\":{{\"usedPercent\":40,\"windowDurationMins\":300,\"resetsAt\":null}}}},\"rateLimitResetCredits\":{{\"availableCount\":1}}}}}}'\nsleep 30\n",
                pid_file.display(),
            ),
        );
        fs::set_permissions(
            codex_home.join("auth.json"),
            fs::Permissions::from_mode(0o600),
        )
        .unwrap();
        let limits =
            read_with_command(&script, &codex_home, Duration::from_secs(2), checked_at()).unwrap();
        assert_eq!(limits.five_hour.unwrap().remaining_percent, 60);
        assert_eq!(
            fs::read_to_string(codex_home.join("auth.json")).unwrap(),
            "refreshed"
        );
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
        let codex_home = temp.path().join("codex-home");
        fs::create_dir(&codex_home).unwrap();
        write_executable_script(&script, "#!/bin/sh\nread a\nread b\nread c\nsleep 30\n");
        let error = read_with_command(
            &script,
            &codex_home,
            Duration::from_millis(100),
            checked_at(),
        )
        .unwrap_err();
        assert_eq!(error, "Codex limits check timed out");
    }
}
