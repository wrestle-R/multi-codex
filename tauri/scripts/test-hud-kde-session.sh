#!/usr/bin/env bash
# Use an isolated compositor, home and session bus, including on hosted runners.
set -euo pipefail
HUD_FIXTURE=$(mktemp -d "${TMPDIR:-/tmp}/mc-orbit-kde.XXXXXX")
cleanup() {
  # Session-bus activation can leave portal/accessibility children writing to
  # this home after the compositor exits. Stop only this fixture's processes.
  python3 - "$HUD_FIXTURE/runtime" <<'PY'
import os, signal, select, sys
from pathlib import Path
runtime = os.fsencode(sys.argv[1])
handles = []
for entry in Path('/proc').iterdir():
    if not entry.name.isdigit() or int(entry.name) in (os.getpid(), os.getppid()):
        continue
    try:
        if entry.stat().st_uid != os.getuid():
            continue
        handle = os.pidfd_open(int(entry.name))
        fields = (entry / 'environ').read_bytes().split(b'\0')
        if b'XDG_RUNTIME_DIR=' + runtime not in fields:
            os.close(handle)
            continue
        signal.pidfd_send_signal(handle, signal.SIGTERM)
        handles.append(handle)
    except (ProcessLookupError, PermissionError, FileNotFoundError):
        continue
for handle in handles:
    if not select.select([handle], [], [], 2)[0]:
        signal.pidfd_send_signal(handle, signal.SIGKILL)
    os.close(handle)
PY
  if mountpoint -q "$HUD_FIXTURE/runtime/doc"; then
    fusermount3 -uz "$HUD_FIXTURE/runtime/doc"
  fi
  rm -rf "$HUD_FIXTURE"
}
trap cleanup EXIT
mkdir -p "$HUD_FIXTURE"/{home,config,data,cache,runtime}
chmod 700 "$HUD_FIXTURE/runtime"
export HOME="$HUD_FIXTURE/home" XDG_CONFIG_HOME="$HUD_FIXTURE/config" XDG_DATA_HOME="$HUD_FIXTURE/data" XDG_CACHE_HOME="$HUD_FIXTURE/cache" XDG_RUNTIME_DIR="$HUD_FIXTURE/runtime"
export XDG_CURRENT_DESKTOP=KDE XDG_SESSION_TYPE=wayland WAYLAND_DISPLAY=multi-codex-test
export GDK_BACKEND=wayland QT_QPA_PLATFORM=offscreen LIBGL_ALWAYS_SOFTWARE=1 NO_AT_BRIDGE=1
export QT_FORCE_STDERR_LOGGING=1
export MULTI_CODEX_TEST_COMPOSITOR_ISOLATED=1
unset HYPRLAND_INSTANCE_SIGNATURE DISPLAY DBUS_SESSION_BUS_ADDRESS
dbus-run-session -- bash <<'SESSION'
set -euo pipefail
kwin_wayland --version > "$HUD_EVIDENCE/environment.txt"
cat /etc/os-release >> "$HUD_EVIDENCE/environment.txt"
kwin_wayland --virtual --width 1280 --height 800 --socket "$WAYLAND_DISPLAY" --no-lockscreen --no-global-shortcuts --no-kactivities > "$HUD_EVIDENCE/compositor.log" 2>&1 &
HUD_KWIN_PID=$!
cleanup() {
  kill "$HUD_KWIN_PID" 2>/dev/null || true
  wait "$HUD_KWIN_PID" 2>/dev/null || true
}
trap cleanup EXIT
for attempt in {1..100}; do
  if [[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]] && busctl --user --no-pager list | grep -q org.kde.KWin; then break; fi
  if ! kill -0 "$HUD_KWIN_PID" 2>/dev/null; then cat "$HUD_EVIDENCE/compositor.log"; exit 1; fi
  sleep .2
done
busctl --user introspect org.kde.KWin /Scripting
python3 tauri/scripts/test-hud-compositor.py --backend kde --launcher "$HUD_LAUNCHER" --output "$HUD_EVIDENCE"
SESSION
