#!/bin/bash
# Run under dbus-run-session; never load the desktop owner's settings.
set -euo pipefail
HUD_FIXTURE=$(mktemp -d)
HUD_GNOME_PID=''
trap 'if [[ -n "$HUD_GNOME_PID" ]]; then kill "$HUD_GNOME_PID" 2>/dev/null || true; wait "$HUD_GNOME_PID" 2>/dev/null || true; fi; rm -rf "$HUD_FIXTURE"' EXIT
mkdir -p "$HUD_FIXTURE/home" "$HUD_FIXTURE/config" "$HUD_FIXTURE/data" "$HUD_FIXTURE/runtime"
chmod 700 "$HUD_FIXTURE/runtime"
export HOME="$HUD_FIXTURE/home" XDG_CONFIG_HOME="$HUD_FIXTURE/config" XDG_DATA_HOME="$HUD_FIXTURE/data" XDG_RUNTIME_DIR="$HUD_FIXTURE/runtime"
export XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland LIBGL_ALWAYS_SOFTWARE=1 GSETTINGS_BACKEND=memory
export MULTI_CODEX_TEST_COMPOSITOR_ISOLATED=1 NO_AT_BRIDGE=1
unset HYPRLAND_INSTANCE_SIGNATURE DISPLAY WAYLAND_DISPLAY
EXTENSION_ID=multicodex-desktops@multicodex.desktop
mkdir -p "$XDG_DATA_HOME/gnome-shell/extensions"
cp -r "tauri/platform/gnome/$EXTENSION_ID" "$XDG_DATA_HOME/gnome-shell/extensions/$EXTENSION_ID"
# Use a private dconf database shared by this shell and its child compositor.
export GSETTINGS_BACKEND=dconf
# dbus-run-session started before the private HOME was set. Services activated
# later (especially dconf) must inherit the same disposable paths as Shell.
export WAYLAND_DISPLAY=wayland-0
dbus-update-activation-environment HOME XDG_CONFIG_HOME XDG_DATA_HOME XDG_RUNTIME_DIR XDG_CURRENT_DESKTOP WAYLAND_DISPLAY
gsettings set org.gnome.shell enabled-extensions "['$EXTENSION_ID']"
gsettings set org.gnome.shell disable-user-extensions false
gsettings get org.gnome.shell enabled-extensions
gnome-shell --wayland --headless --virtual-monitor=1280x800 --unsafe-mode > "$HUD_EVIDENCE/compositor.log" 2>&1 &
HUD_GNOME_PID=$!
for attempt in {1..100}; do
  if busctl --user --no-pager list | grep -q com.multicodex.Desktops; then break; fi
  if ! kill -0 "$HUD_GNOME_PID" 2>/dev/null; then cat "$HUD_EVIDENCE/compositor.log"; exit 1; fi
  sleep .2
done
busctl --user introspect com.multicodex.Desktops /com/multicodex/Desktops
export WAYLAND_DISPLAY=wayland-0
python3 tauri/scripts/test-hud-compositor.py --backend gnome --launcher "$HUD_LAUNCHER" --output "$HUD_EVIDENCE"
