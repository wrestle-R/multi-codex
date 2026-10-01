#!/usr/bin/env bash
# Run the native bridge against an isolated virtual KWin compositor, never the user's desktop.
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
command -v kwin_wayland >/dev/null
command -v dbus-run-session >/dev/null
TASK_DIR=$(mktemp -d)
trap 'rm -rf "$TASK_DIR"' EXIT
mkdir -p "$TASK_DIR/runtime" "$TASK_DIR/config" "$TASK_DIR/data" "$TASK_DIR/cache"
chmod 700 "$TASK_DIR/runtime"
cat > "$TASK_DIR/config/kwinrc" <<'CONFIG'
[Desktops]
Number=2
Rows=1
CONFIG
cat > "$TASK_DIR/window.py" <<'PY'
import gi
gi.require_version('Gtk', '4.0')
from gi.repository import Gtk
app = Gtk.Application(application_id='com.multicodex.BridgeTest')
def activate(app):
    window = Gtk.ApplicationWindow(application=app, title='Multi Codex bridge test')
    window.set_default_size(320, 200)
    window.present()
app.connect('activate', activate)
app.run([])
PY
export MULTI_CODEX_TEST_ROOT="$ROOT_DIR" MULTI_CODEX_TEST_DIR="$TASK_DIR"
if ! env -u HYPRLAND_INSTANCE_SIGNATURE -u DISPLAY XDG_CURRENT_DESKTOP=KDE WAYLAND_DISPLAY=multi-codex-test GDK_BACKEND=wayland GTK_USE_PORTAL=0 NO_AT_BRIDGE=1 XDG_RUNTIME_DIR="$TASK_DIR/runtime" XDG_CONFIG_HOME="$TASK_DIR/config" XDG_DATA_HOME="$TASK_DIR/data" XDG_CACHE_HOME="$TASK_DIR/cache" dbus-run-session -- bash <<'SESSION' 2>"$TASK_DIR/session.log"
set -euo pipefail
kwin_wayland --virtual --socket multi-codex-test --no-lockscreen --no-global-shortcuts --no-kactivities >"$MULTI_CODEX_TEST_DIR/kwin.log" 2>&1 &
KWIN_TEST_PID=$!
GTK_TEST_PID=""
APP_TEST_PID=""
cleanup() {
  [[ -z "$APP_TEST_PID" ]] || kill "$APP_TEST_PID" 2>/dev/null || true
  [[ -z "$GTK_TEST_PID" ]] || kill "$GTK_TEST_PID" 2>/dev/null || true
  kill "$KWIN_TEST_PID" 2>/dev/null || true
  wait "$KWIN_TEST_PID" 2>/dev/null || true
}
trap cleanup EXIT
for ((attempt=0; attempt<100; attempt++)); do
  [[ ! -S "$XDG_RUNTIME_DIR/multi-codex-test" ]] || break
  kill -0 "$KWIN_TEST_PID" || { cat "$MULTI_CODEX_TEST_DIR/kwin.log"; exit 1; }
  sleep 0.1
done
[[ -S "$XDG_RUNTIME_DIR/multi-codex-test" ]] || { cat "$MULTI_CODEX_TEST_DIR/kwin.log"; exit 1; }
env WAYLAND_DISPLAY=multi-codex-test GDK_BACKEND=wayland /usr/bin/python3 "$MULTI_CODEX_TEST_DIR/window.py" >"$MULTI_CODEX_TEST_DIR/gtk.log" 2>&1 &
GTK_TEST_PID=$!
if [[ -n "${MULTI_CODEX_TEST_APPIMAGE:-}" ]]; then
  env -u HYPRLAND_INSTANCE_SIGNATURE -u DISPLAY WAYLAND_DISPLAY=multi-codex-test GDK_BACKEND=wayland XDG_CURRENT_DESKTOP=KDE "$MULTI_CODEX_TEST_APPIMAGE" --appimage-extract-and-run >"$MULTI_CODEX_TEST_DIR/app.log" 2>&1 &
  APP_TEST_PID=$!
  sleep 3
  kill -0 "$APP_TEST_PID" || { cat "$MULTI_CODEX_TEST_DIR/app.log"; exit 1; }
fi
sleep 1
MULTI_CODEX_TEST_KDE=1 cargo test --manifest-path "$MULTI_CODEX_TEST_ROOT/src-tauri/Cargo.toml" live_kde_inventory_and_placement -- --ignored --nocapture
SESSION
then
  cat "$TASK_DIR/session.log" >&2
  exit 1
fi
