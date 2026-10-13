use serde_json::Value;

#[derive(Debug)]
pub struct CliError {
    pub code: i32,
    pub message: String,
    pub details: Option<Value>,
}

impl CliError {
    pub fn new(code: i32, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }
}

impl From<String> for CliError {
    fn from(message: String) -> Self {
        let lower = message.to_lowercase();
        let code = if lower.contains("busy")
            || lower.contains("lease")
            || lower.contains("close this profile")
            || lower.contains("preview")
        {
            4
        } else if lower.contains("credential store")
            || lower.contains("keyring")
            || lower.contains("not found. install")
            || lower.contains("unavailable")
            || lower.contains("required")
        {
            3
        } else if lower.contains("sign in")
            || lower.contains("authentication")
            || lower.contains("network")
            || lower.contains("logged-in")
        {
            5
        } else {
            1
        };
        Self::new(code, message)
    }
}

impl From<std::io::Error> for CliError {
    fn from(error: std::io::Error) -> Self {
        Self::new(
            if error.kind() == std::io::ErrorKind::PermissionDenied {
                3
            } else {
                1
            },
            format!("Local file operation failed: {error}"),
        )
    }
}

impl From<serde_json::Error> for CliError {
    fn from(_: serde_json::Error) -> Self {
        Self::new(
            2,
            "The input must contain valid JSON; credential contents are not shown.",
        )
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
