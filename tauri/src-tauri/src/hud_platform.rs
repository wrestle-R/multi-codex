//! Compositor owns stacking/fullscreen/Wayland shortcuts. Failure hides the HUD.
use serde_json::Value;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[cfg(target_os = "linux")]
struct Control(Arc<AtomicBool>);
#[cfg(target_os = "linux")]
#[zbus::interface(name = "com.multicodex.Hud1")]
impl Control {
    fn toggle(&self) {
        self.0.fetch_xor(true, Ordering::SeqCst);
    }
}

#[cfg(target_os = "linux")]
pub fn shortcut(hidden: Arc<AtomicBool>) {
    std::thread::spawn(move || {
        let Ok(_connection) = zbus::blocking::connection::Builder::session()
            .and_then(|b| b.name("com.multicodex.Hud"))
            .and_then(|b| b.serve_at("/com/multicodex/Hud", Control(hidden)))
            .and_then(|b| b.build())
        else {
            return;
        };
        if crate::desktop_environment::backend() == "hyprland" {
            // Never replace an existing user binding. This binding is runtime
            // only; no compositor config file is edited.
            if let Ok(bindings) = json_command("binds") {
                let occupied = bindings.as_array().is_none_or(|items| {
                    items.iter().any(|v| {
                        v["modmask"] == 65
                            && v["key"]
                                .as_str()
                                .is_some_and(|k| k.eq_ignore_ascii_case("h"))
                    })
                });
                if !occupied {
                    let _ = run(Command::new("hyprctl").args(["eval", "if not _G.multi_codex_hud_bind then _G.multi_codex_hud_bind = hl.bind('SUPER + SHIFT + H', hl.dsp.exec_cmd('busctl --user call com.multicodex.Hud /com/multicodex/Hud com.multicodex.Hud1 Toggle')) end"]));
                }
            }
        }
        if crate::desktop_environment::backend() == "kde" {
            let _ = kde_shortcut();
        }
        loop {
            std::thread::park();
        }
    });
}
#[cfg(not(target_os = "linux"))]
pub fn shortcut(_hidden: Arc<AtomicBool>) {}

#[cfg(target_os = "linux")]
fn kde_shortcut() -> Result<(), String> {
    let connection = zbus::blocking::Connection::session().map_err(|e| e.to_string())?;
    let scripting = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        "/Scripting",
        "org.kde.kwin.Scripting",
    )
    .map_err(|e| e.to_string())?;
    let mut file = tempfile::Builder::new()
        .prefix("multi-codex-hud-shortcut-")
        .suffix(".js")
        .tempfile()
        .map_err(|e| e.to_string())?;
    use std::io::Write;
    file.write_all(b"registerShortcut('MultiCodexHud', 'Show or hide Multi Codex Orbit', 'Meta+Shift+H', function(){ callDBus('com.multicodex.Hud','/com/multicodex/Hud','com.multicodex.Hud1','Toggle'); });").map_err(|e| e.to_string())?;
    let index: i32 = scripting
        .call(
            "loadScript",
            &(
                file.path().to_string_lossy().as_ref(),
                "multi-codex-hud-shortcut",
            ),
        )
        .map_err(|e| e.to_string())?;
    if index < 0 {
        return Err("KDE shortcut unavailable".into());
    }
    let path = format!("/Scripting/Script{index}");
    let script = zbus::blocking::Proxy::new(
        &connection,
        "org.kde.KWin",
        path.as_str(),
        "org.kde.kwin.Script",
    )
    .map_err(|e| e.to_string())?;
    script
        .call::<_, _, ()>("run", &())
        .map_err(|e| e.to_string())
}
#[cfg(target_os = "linux")]
fn run(command: &mut Command) -> Result<String, String> {
    let output = crate::process::output(command, Duration::from_secs(2))?;
    if !output.status.success() {
        return Err("HUD compositor request unavailable".into());
    }
    String::from_utf8(output.stdout).map_err(|_| "Invalid compositor reply".into())
}
#[cfg(target_os = "linux")]
fn json_command(command: &str) -> Result<Value, String> {
    serde_json::from_str(&run(Command::new("hyprctl").args(["-j", command]))?)
        .map_err(|_| "Invalid compositor reply".into())
}

#[cfg(target_os = "linux")]
fn gnome(request: Value) -> Result<Value, String> {
    let connection = zbus::blocking::connection::Builder::session()
        .map_err(|e| e.to_string())?
        .method_timeout(Duration::from_secs(2))
        .build()
        .map_err(|e| e.to_string())?;
    let proxy = zbus::blocking::Proxy::new(
        &connection,
        "com.multicodex.Desktops",
        "/com/multicodex/Desktops",
        "com.multicodex.Desktops1",
    )
    .map_err(|e| e.to_string())?;
    let raw: String = proxy
        .call("Hud", &(request.to_string(),))
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&raw).map_err(|e| e.to_string())
}
/// True only with an authoritative non-fullscreen result for this display.
#[cfg(target_os = "linux")]
pub fn visible(x: f64, y: f64) -> bool {
    match crate::desktop_environment::backend() {
        "hyprland" => hyprland_visible(x, y).unwrap_or(false),
        "gnome" => gnome(serde_json::json!({"pid":std::process::id(),"x":x,"y":y}))
            .is_ok_and(|state| state["supported"] == true && state["fullscreen"] == false),
        "kde" => crate::linux_desktops::hud_state(std::process::id(), x, y)
            .is_ok_and(|state| state["supported"] == true && state["fullscreen"] == false),
        _ => false,
    }
}
#[cfg(target_os = "linux")]
fn hyprland_visible(x: f64, y: f64) -> Result<bool, String> {
    let monitors = json_command("monitors")?;
    let monitor = monitors
        .as_array()
        .ok_or("Missing display inventory")?
        .iter()
        .find(|m| {
            let scale = m["scale"].as_f64().unwrap_or(1.0);
            let mx = m["x"].as_f64().unwrap_or(0.0);
            let my = m["y"].as_f64().unwrap_or(0.0);
            x >= mx
                && y >= my
                && x < mx + m["width"].as_f64().unwrap_or(0.0) / scale
                && y < my + m["height"].as_f64().unwrap_or(0.0) / scale
        })
        .ok_or("HUD display no longer exists")?;
    let clients = json_command("clients")?;
    let clients = clients.as_array().ok_or("Missing window inventory")?;
    for window in clients {
        if window["pid"] == std::process::id() && window["title"] == "Multi Codex Orbit" {
            if let Some(address) = window["address"]
                .as_str()
                .filter(|a| a.starts_with("0x") && a[2..].chars().all(|c| c.is_ascii_hexdigit()))
            {
                if window["floating"] != true {
                    let _ = run(Command::new("hyprctl").args(["eval", &format!("local w=hl.get_window('address:{address}'); if w then hl.dispatch(hl.dsp.window.float({{action='set',window=w}})) end")]));
                }
                if window["pinned"] != true {
                    let _ = run(Command::new("hyprctl").args(["eval", &format!("local w=hl.get_window('address:{address}'); if w then hl.dispatch(hl.dsp.window.pin({{action='set',window=w}})) end")]));
                }
            }
        }
    }
    Ok(!clients.iter().any(|window| {
        window["mapped"] != false
            && window["monitor"] == monitor["id"]
            && window["fullscreen"].as_u64().is_some_and(|v| v & 2 != 0)
            && (window["workspace"]["id"] == monitor["activeWorkspace"]["id"]
                || window["workspace"]["id"] == monitor["specialWorkspace"]["id"])
    }))
}
#[cfg(target_os = "macos")]
pub fn visible(_x: f64, _y: f64) -> bool {
    macos::ordinary_space()
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn visible(_x: f64, _y: f64) -> bool {
    false
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::{c_char, c_void};
    #[link(name = "objc")]
    extern "C" {
        fn objc_getClass(name: *const c_char) -> *mut c_void;
        fn sel_registerName(name: *const c_char) -> *mut c_void;
        fn objc_msgSend();
    }
    pub fn ordinary_space() -> bool {
        unsafe {
            let send: unsafe extern "C" fn(*mut c_void, *mut c_void) -> *mut c_void =
                std::mem::transmute(objc_msgSend as *const ());
            let app = send(
                objc_getClass(c"NSApplication".as_ptr()),
                sel_registerName(c"sharedApplication".as_ptr()),
            );
            let options: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
                std::mem::transmute(objc_msgSend as *const ());
            // Fullscreen presentation is owned by AppKit; the HUD does not join
            // another application's fullscreen Spaces.
            options(
                app,
                sel_registerName(c"currentSystemPresentationOptions".as_ptr()),
            ) & (1 << 10)
                == 0
        }
    }
    pub unsafe fn configure(window: *mut c_void) {
        let send: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) =
            std::mem::transmute(objc_msgSend as *const ());
        // CanJoinAllSpaces + IgnoresCycle; FullScreenAuxiliary intentionally absent.
        send(
            window,
            sel_registerName(c"setCollectionBehavior:".as_ptr()),
            1 | 64,
        );
        send(window, sel_registerName(c"setLevel:".as_ptr()), 3);
    }
}
pub fn configure(window: &tauri::WebviewWindow) {
    #[cfg(target_os = "macos")]
    if let Ok(native) = window.ns_window() {
        unsafe {
            macos::configure(native);
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

pub fn geometry(x: f64, y: f64, width: f64, height: f64) {
    #[cfg(target_os = "linux")]
    if crate::desktop_environment::backend() == "hyprland" {
        if let Ok(clients) = json_command("clients") {
            if let Some(window) = clients.as_array().and_then(|windows| {
                windows
                    .iter()
                    .find(|w| w["pid"] == std::process::id() && w["title"] == "Multi Codex Orbit")
            }) {
                if let Some(address) = window["address"].as_str().filter(|a| {
                    a.starts_with("0x") && a[2..].chars().all(|c| c.is_ascii_hexdigit())
                }) {
                    let script = format!("local w=hl.get_window('address:{address}'); if w then hl.dispatch(hl.dsp.window.float({{action='set',window=w}})); hl.dispatch(hl.dsp.window.move({{x={x},y={y},relative=false,window=w}})); hl.dispatch(hl.dsp.window.resize({{x={width},y={height},relative=false,window=w}})) end");
                    let _ = run(Command::new("hyprctl").args(["eval", &script]));
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let rectangle = serde_json::json!({"x":x,"y":y,"width":width,"height":height});
        match crate::desktop_environment::backend() {
            "gnome" => {
                let _ = gnome(
                    serde_json::json!({"pid":std::process::id(),"x":x,"y":y,"rectangle":rectangle}),
                );
            }
            "kde" => {
                let _ =
                    crate::linux_desktops::hud_operation(std::process::id(), x, y, Some(rectangle));
            }
            _ => {}
        }
    }
    #[cfg(not(target_os = "linux"))]
    let _ = (x, y, width, height);
}
pub fn position(window: &tauri::WebviewWindow, scale: f64) -> Option<(f64, f64)> {
    #[cfg(target_os = "linux")]
    if crate::desktop_environment::backend() == "hyprland" {
        let clients = json_command("clients").ok()?;
        let client = clients
            .as_array()?
            .iter()
            .find(|w| w["pid"] == std::process::id() && w["title"] == "Multi Codex Orbit")?;
        return Some((client["at"][0].as_f64()?, client["at"][1].as_f64()?));
    }
    #[cfg(target_os = "linux")]
    {
        let state = match crate::desktop_environment::backend() {
            "gnome" => Some(gnome(serde_json::json!({"pid":std::process::id(),"x":0,"y":0})).ok()?),
            "kde" => Some(crate::linux_desktops::hud_state(std::process::id(), 0.0, 0.0).ok()?),
            _ => None,
        };
        if let Some(state) = state {
            return Some((
                state["position"]["x"].as_f64()?,
                state["position"]["y"].as_f64()?,
            ));
        }
    }
    window.outer_position().ok().map(|position| {
        let position = position.to_logical::<f64>(scale);
        (position.x, position.y)
    })
}

/// Remove only shortcuts created by this process; never edit user config.
pub fn cleanup() {
    #[cfg(target_os = "linux")]
    match crate::desktop_environment::backend() {
        "hyprland" => {
            let _ = run(Command::new("hyprctl").args(["eval", "if _G.multi_codex_hud_bind then _G.multi_codex_hud_bind:remove(); _G.multi_codex_hud_bind=nil end"]));
        }
        "kde" => {
            if let Ok(connection) = zbus::blocking::Connection::session() {
                if let Ok(proxy) = zbus::blocking::Proxy::new(
                    &connection,
                    "org.kde.KWin",
                    "/Scripting",
                    "org.kde.kwin.Scripting",
                ) {
                    let _: Result<bool, _> =
                        proxy.call("unloadScript", &("multi-codex-hud-shortcut",));
                }
            }
        }
        _ => {}
    }
}
