//! Versioned GNOME extension and on-demand Plasma 6 KWin script adapters.
use crate::desktop_environment::{unsupported_inventory, DesktopInventory};
use crate::profiles::Result;
use std::io::Write;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;
use zbus::blocking::{Connection, Proxy};

const TIMEOUT: Duration = Duration::from_secs(3);
const GNOME_HELP: &str = "Install and enable Multi Codex desktops using the bundled platform/gnome extension, then refresh. See docs/platform-support.md.";

fn connection() -> Result<Connection> {
    zbus::blocking::connection::Builder::session()
        .map_err(|_| "The desktop session bus is unavailable")?
        .method_timeout(TIMEOUT)
        .build()
        .map_err(|_| "The desktop session bus is unavailable".into())
}

fn parse_snapshot(raw: &str, backend: &str) -> Result<DesktopInventory> {
    let value: serde_json::Value =
        serde_json::from_str(raw).map_err(|_| "Desktop bridge returned invalid JSON")?;
    if let Some(error) = value.get("error").and_then(|v| v.as_str()) {
        return Err(error.into());
    }
    let snapshot: DesktopInventory = serde_json::from_value(value)
        .map_err(|_| "Desktop bridge returned an invalid inventory")?;
    if snapshot.protocol_version != 1 || snapshot.capabilities.backend != backend {
        return Err("Desktop bridge protocol is incompatible. Update the integration.".into());
    }
    if snapshot
        .desktops
        .iter()
        .any(|d| !d.id.starts_with(&format!("{backend}:")))
    {
        return Err("Desktop bridge returned invalid desktop identifiers".into());
    }
    Ok(snapshot)
}

pub fn inventory(backend: &str) -> Result<DesktopInventory> {
    let result = if backend == "gnome" {
        gnome_inventory()
    } else {
        kde_request(None)
    };
    // Missing bridges leave the current-desktop launch usable and expose actionable guidance.
    Ok(result.unwrap_or_else(|error| unsupported_inventory(backend, format!("{error}. {}", if backend == "gnome" { GNOME_HELP } else { "KDE Plasma 6 with its KWin scripting service is required. See docs/platform-support.md." }))))
}

fn gnome_inventory() -> Result<DesktopInventory> {
    let connection = connection()?;
    let proxy = Proxy::new(
        &connection,
        "com.multicodex.Desktops",
        "/com/multicodex/Desktops",
        "com.multicodex.Desktops1",
    )
    .map_err(|_| GNOME_HELP)?;
    let raw: String = proxy.call("Inventory", &()).map_err(|_| GNOME_HELP)?;
    parse_snapshot(&raw, "gnome")
}

pub fn move_window(backend: &str, id: &str, destination: &str) -> Result<()> {
    if backend == "kde" {
        kde_request(Some((id, destination)))?;
        return Ok(());
    }
    let connection = connection()?;
    let proxy = Proxy::new(
        &connection,
        "com.multicodex.Desktops",
        "/com/multicodex/Desktops",
        "com.multicodex.Desktops1",
    )
    .map_err(|_| GNOME_HELP)?;
    let result: String = proxy
        .call("Move", &(id, destination))
        .map_err(|e| format!("GNOME could not move this window: {e}"))?;
    if result == "ok" {
        Ok(())
    } else {
        Err("GNOME rejected the window operation".into())
    }
}

type ReplyState = Arc<(Mutex<Option<String>>, Condvar)>;
struct KWinReply {
    nonce: String,
    state: ReplyState,
}
#[zbus::interface(name = "com.multicodex.KWinReply1")]
impl KWinReply {
    fn publish(&self, nonce: &str, snapshot: &str) -> zbus::fdo::Result<()> {
        if nonce != self.nonce || snapshot.len() > 4 * 1024 * 1024 {
            return Err(zbus::fdo::Error::InvalidArgs(
                "Invalid desktop reply".into(),
            ));
        }
        let (lock, changed) = &*self.state;
        *lock
            .lock()
            .map_err(|_| zbus::fdo::Error::Failed("Reply state unavailable".into()))? =
            Some(snapshot.into());
        changed.notify_one();
        Ok(())
    }
}

fn kde_request(operation: Option<(&str, &str)>) -> Result<DesktopInventory> {
    let nonce = uuid::Uuid::new_v4().to_string();
    let state: ReplyState = Arc::new((Mutex::new(None), Condvar::new()));
    let connection = zbus::blocking::connection::Builder::session()
        .map_err(|_| "The desktop session bus is unavailable")?
        .method_timeout(TIMEOUT)
        .serve_at(
            "/com/multicodex/KWinReply",
            KWinReply {
                nonce: nonce.clone(),
                state: Arc::clone(&state),
            },
        )
        .map_err(|e| e.to_string())?
        .build()
        .map_err(|e| e.to_string())?;
    let callback = connection
        .unique_name()
        .ok_or("Desktop callback name unavailable")?
        .as_str();
    let (action, window, destination) =
        operation
            .map(|(w, d)| ("move", w, d))
            .unwrap_or(("inventory", "", ""));
    let request = serde_json::json!({ "callback": callback, "nonce": nonce, "action": action, "window": window, "destination": destination });
    let mut file = tempfile::Builder::new()
        .prefix("multi-codex-kwin-")
        .suffix(".js")
        .tempfile()
        .map_err(|e| e.to_string())?;
    writeln!(
        file,
        "const request = {request};\n{}",
        include_str!("../../platform/kde/bridge.js")
    )
    .map_err(|e| e.to_string())?;
    let scripting = Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .map_err(|e| e.to_string())?;
    let name = format!("multi-codex-{nonce}");
    let index: i32 = scripting
        .call(
            "loadScript",
            &(file.path().to_string_lossy().as_ref(), name.as_str()),
        )
        .map_err(|e| format!("Could not load the KWin desktop bridge: {e}"))?;
    if index < 0 {
        return Err("KWin rejected the desktop script".into());
    }
    let result = (|| {
        let path = format!("/Scripting/Script{index}");
        let script = Proxy::new(
            &connection,
            "org.kde.KWin",
            path.as_str(),
            "org.kde.kwin.Script",
        )
        .map_err(|e| e.to_string())?;
        let _: () = script
            .call("run", &())
            .map_err(|e| format!("Could not start the KWin desktop bridge: {e}"))?;
        let (lock, changed) = &*state;
        let reply = lock.lock().map_err(|_| "Desktop reply state unavailable")?;
        let (reply, _) = changed
            .wait_timeout_while(reply, TIMEOUT, |value| value.is_none())
            .map_err(|_| "Desktop reply state unavailable")?;
        let raw = reply
            .as_ref()
            .ok_or("KWin desktop bridge timed out. Plasma 6 is required.")?;
        parse_snapshot(raw, "kde")
    })();
    let _: std::result::Result<bool, _> = scripting.call("unloadScript", &(name.as_str(),));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_incompatible_protocol_and_forwards_bridge_errors() {
        assert!(parse_snapshot(r#"{"protocolVersion":2,"capabilities":{"backend":"gnome","enumerateDesktops":true,"enumerateWindows":true,"moveWindows":true,"reason":null},"desktops":[]}"#, "gnome").is_err());
        assert_eq!(
            parse_snapshot(r#"{"error":"Permission denied"}"#, "kde").unwrap_err(),
            "Permission denied"
        );
    }
}
