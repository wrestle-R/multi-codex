//! Hyprland HUDs use layer-shell, so no managed application window participates
//! in tiling, focus, resizing or workspace animations.
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
use tauri::WebviewWindow;

static ACTIVE: AtomicBool = AtomicBool::new(false);
static VISIBLE: AtomicBool = AtomicBool::new(false);
static RECTANGLE: Mutex<Option<(f64, f64, f64, f64)>> = Mutex::new(None);
#[derive(Clone, serde::Deserialize)]
pub struct InputRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}
static INPUT: Mutex<Vec<InputRectangle>> = Mutex::new(Vec::new());
pub fn active() -> bool {
    ACTIVE.load(Ordering::SeqCst)
}

#[cfg(target_os = "linux")]
mod native {
    use super::*;
    use gtk::{glib::translate::ToGlibPtr, prelude::*};
    use std::cell::RefCell;
    use std::ffi::{c_char, c_void};
    thread_local! { static SURFACE: RefCell<Option<gtk::ApplicationWindow>> = const { RefCell::new(None) }; }
    #[link(name = "gtk-layer-shell")]
    extern "C" {
        fn gtk_layer_is_supported() -> i32;
        fn gtk_layer_init_for_window(window: *mut c_void);
        fn gtk_layer_is_layer_window(window: *mut c_void) -> i32;
        fn gtk_layer_set_namespace(window: *mut c_void, namespace: *const c_char);
        fn gtk_layer_set_layer(window: *mut c_void, layer: i32);
        fn gtk_layer_set_keyboard_mode(window: *mut c_void, mode: i32);
        fn gtk_layer_set_exclusive_zone(window: *mut c_void, zone: i32);
        fn gtk_layer_set_anchor(window: *mut c_void, edge: i32, enabled: i32);
        fn gtk_layer_set_margin(window: *mut c_void, edge: i32, margin: i32);
        fn gtk_layer_set_monitor(window: *mut c_void, monitor: *mut c_void);
    }
    pub fn setup(window: &WebviewWindow) -> Result<(), String> {
        let original = window.gtk_window().map_err(|e| e.to_string())?;
        let application = original
            .application()
            .ok_or("Orbit GTK application unavailable")?;
        let gtk = gtk::ApplicationWindow::new(&application);
        let ptr: *mut gtk::ffi::GtkApplicationWindow = gtk.to_glib_none().0;
        let ptr = ptr.cast();
        unsafe {
            if gtk_layer_is_supported() == 0 {
                return Err("Hyprland's layer-shell protocol is unavailable".into());
            }
            gtk_layer_init_for_window(ptr);
            if gtk_layer_is_layer_window(ptr) == 0 {
                return Err("Could not create the Orbit overlay".into());
            }
            gtk_layer_set_namespace(ptr, c"multi-codex-orbit".as_ptr());
            gtk_layer_set_layer(ptr, 3); // overlay, hidden explicitly for fullscreen
            gtk_layer_set_keyboard_mode(ptr, 0); // never steal typing focus
            gtk_layer_set_exclusive_zone(ptr, -1); // do not rearrange other windows
            gtk_layer_set_anchor(ptr, 0, 1); // left
            gtk_layer_set_anchor(ptr, 2, 1); // top
        }
        gtk.set_title("Multi Codex Orbit");
        gtk.set_decorated(false);
        gtk.set_resizable(false);
        gtk.set_accept_focus(false);
        gtk.set_focus_on_map(false);
        gtk.set_app_paintable(true);
        if let Some(visual) =
            gtk::prelude::WidgetExt::screen(&gtk).and_then(|screen| screen.rgba_visual())
        {
            gtk.set_visual(Some(&visual));
        }
        let style = gtk::CssProvider::new();
        style
            .load_from_data(b"window { background-color: transparent; }")
            .map_err(|e| e.to_string())?;
        gtk.style_context()
            .add_provider(&style, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
        // Tauri/Wry realize their own GTK toplevel before it is exposed. Move
        // the retained webview container into a fresh, unrealized layer window
        // instead of invalidating Wry's GDK references with unrealize().
        let contents = window.default_vbox().map_err(|e| e.to_string())?;
        original.remove(&contents);
        gtk.add(&contents);
        original.hide();
        SURFACE.with(|surface| *surface.borrow_mut() = Some(gtk));
        ACTIVE.store(true, Ordering::SeqCst);
        Ok(())
    }
    fn apply_input(gtk: &gtk::ApplicationWindow, rectangles: &[InputRectangle]) {
        if let Some(gdk) = gtk.window() {
            let region = cairo::Region::create();
            for r in rectangles {
                let _ = region.union_rectangle(&cairo::RectangleInt::new(
                    r.x.floor() as i32,
                    r.y.floor() as i32,
                    r.width.ceil() as i32,
                    r.height.ceil() as i32,
                ));
            }
            gdk.input_shape_combine_region(&region, 0, 0);
        }
    }
    pub fn geometry(window: &WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
        let _ = window.run_on_main_thread(move || {
            let Some(gtk) = SURFACE.with(|surface| surface.borrow().clone()) else {
                return;
            };
            let display = gtk.display();
            let Some(monitor) = display.monitor_at_point(x as i32, y as i32) else {
                return;
            };
            let origin = monitor.geometry();
            let ptr: *mut gtk::ffi::GtkApplicationWindow = gtk.to_glib_none().0;
            let monitor_ptr: *mut gtk::gdk::ffi::GdkMonitor = monitor.to_glib_none().0;
            unsafe {
                gtk_layer_set_monitor(ptr.cast(), monitor_ptr.cast());
                gtk_layer_set_margin(ptr.cast(), 0, (x - f64::from(origin.x())).round() as i32);
                gtk_layer_set_margin(ptr.cast(), 2, (y - f64::from(origin.y())).round() as i32);
            }
            gtk.set_size_request(width.ceil() as i32, height.ceil() as i32);
            gtk.resize(1, 1);
            if let Ok(mut rect) = RECTANGLE.lock() {
                if let Some((old_x, old_y, _, _)) = *rect {
                    if let Ok(mut input) = INPUT.lock() {
                        for r in input.iter_mut() {
                            r.x += old_x - x;
                            r.y += old_y - y;
                        }
                        apply_input(&gtk, &input);
                    }
                }
                *rect = Some((x, y, width, height));
            }
        });
    }
    pub fn show(window: &WebviewWindow, visible: bool) {
        let _ = window.run_on_main_thread(move || {
            SURFACE.with(|surface| {
                if let Some(gtk) = surface.borrow().as_ref() {
                    if visible {
                        gtk.show_all();
                        if let Ok(input) = INPUT.lock() {
                            apply_input(gtk, &input);
                        }
                    } else {
                        gtk.hide();
                    }
                    VISIBLE.store(visible, Ordering::SeqCst);
                }
            })
        });
    }
    pub fn input(window: &WebviewWindow, rectangles: Vec<InputRectangle>) {
        let _ = window.run_on_main_thread(move || {
            SURFACE.with(|surface| {
                if let Some(gtk) = surface.borrow().as_ref() {
                    apply_input(gtk, &rectangles);
                }
            })
        });
    }
}

pub fn setup(window: &WebviewWindow) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    if crate::desktop_environment::backend() == "hyprland" {
        return native::setup(window);
    }
    let _ = window;
    Ok(())
}
pub fn geometry(window: &WebviewWindow, x: f64, y: f64, width: f64, height: f64) {
    #[cfg(target_os = "linux")]
    if active() {
        native::geometry(window, x, y, width, height);
    }
    let _ = (window, x, y, width, height);
}
pub fn rectangle() -> Option<(f64, f64, f64, f64)> {
    *RECTANGLE.lock().ok()?
}
pub fn input(window: &WebviewWindow, rectangles: Vec<InputRectangle>) -> Result<(), String> {
    if rectangles.len() > 4
        || rectangles.iter().any(|r| {
            ![r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
                || r.x < 0.0
                || r.y < 0.0
                || r.width < 0.0
                || r.height < 0.0
                || r.x + r.width > 4096.0
                || r.y + r.height > 4096.0
        })
    {
        return Err("Invalid Orbit input bounds".into());
    }
    if let Ok(mut input) = INPUT.lock() {
        *input = rectangles.clone();
    }
    #[cfg(target_os = "linux")]
    if active() {
        native::input(window, rectangles);
    }
    let _ = window;
    Ok(())
}
pub fn is_visible(window: &WebviewWindow) -> bool {
    if active() {
        VISIBLE.load(Ordering::SeqCst)
    } else {
        window.is_visible().unwrap_or(false)
    }
}
pub fn show(window: &WebviewWindow, visible: bool) {
    #[cfg(target_os = "linux")]
    if active() {
        native::show(window, visible);
        return;
    }
    if visible {
        let _ = window.show();
    } else {
        let _ = window.hide();
    }
}
