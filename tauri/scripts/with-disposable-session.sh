#!/usr/bin/env bash
# Keep accessibility/portal sockets in the fixture runtime, not the user's.
set -euo pipefail
original_runtime="${XDG_RUNTIME_DIR:?A logged-in Wayland session is required}"
original_display="${WAYLAND_DISPLAY:?A Wayland display is required}"
if [[ "$original_display" != /* ]]; then original_display="$original_runtime/$original_display"; fi
fixture_runtime="$(mktemp -d "${TMPDIR:-/tmp}/mc-orbit-runtime.XXXXXX")"
cleanup() {
  if mountpoint -q "$fixture_runtime/doc"; then fusermount3 -u "$fixture_runtime/doc" || true; fi
  rm -rf -- "$fixture_runtime"
}
trap cleanup EXIT
original_hyprctl="$(command -v hyprctl)"
mkdir "$fixture_runtime/bin"
mkdir -p "$fixture_runtime/home" "$fixture_runtime/config" "$fixture_runtime/data" "$fixture_runtime/cache"
cat > "$fixture_runtime/bin/hyprctl" <<'WRAPPER'
#!/bin/sh
exec env XDG_RUNTIME_DIR="$MULTI_CODEX_TEST_COMPOSITOR_RUNTIME" "$MULTI_CODEX_TEST_HYPRCTL" "$@"
WRAPPER
chmod 700 "$fixture_runtime/bin/hyprctl"
cat > "$fixture_runtime/run" <<'RUNNER'
#!/bin/sh
set -eu
if [ "${MULTI_CODEX_TEST_UNLOCK_KEYRING:-0}" = 1 ]; then
  # This new keyring belongs only to the disposable HOME and private bus.
  printf '%s' 'disposable-fixture-only' | gnome-keyring-daemon --unlock --components=secrets >/dev/null
fi
exec "$@"
RUNNER
chmod 700 "$fixture_runtime/run"
XDG_RUNTIME_DIR="$fixture_runtime" WAYLAND_DISPLAY="$original_display" PATH="$fixture_runtime/bin:$PATH" \
HOME="$fixture_runtime/home" XDG_CONFIG_HOME="$fixture_runtime/config" XDG_DATA_HOME="$fixture_runtime/data" XDG_CACHE_HOME="$fixture_runtime/cache" \
MULTI_CODEX_TEST_COMPOSITOR_RUNTIME="$original_runtime" MULTI_CODEX_TEST_HYPRCTL="$original_hyprctl" \
  dbus-run-session -- env MULTI_CODEX_TEST_HUD_ISOLATED_BUS=1 NO_AT_BRIDGE=1 "$fixture_runtime/run" "$@"
