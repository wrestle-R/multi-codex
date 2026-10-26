#!/usr/bin/env bash
set -euo pipefail

# AppImageHub starts Xvfb directly, without xvfb-run's Xauthority cookie.
# Let Xvfb reserve a free display atomically and keep its lifetime bounded.
display_root="$(mktemp -d)"
Xvfb -displayfd 3 -screen 0 800x600x24 -nolisten tcp 3>"$display_root/number" >"$display_root/xvfb.log" 2>&1 &
display_pid=$!
cleanup() {
  local result=$?
  kill "$display_pid" 2>/dev/null || true
  wait "$display_pid" 2>/dev/null || true
  rm -rf "$display_root"
  exit "$result"
}
trap cleanup EXIT
for _ in 1 2 3 4 5; do
  [[ ! -s "$display_root/number" ]] || break
  kill -0 "$display_pid" 2>/dev/null || { cat "$display_root/xvfb.log" >&2; exit 1; }
  sleep 1
done
display_number="$(cat "$display_root/number")"
[[ "$display_number" =~ ^[0-9]+$ ]] || { cat "$display_root/xvfb.log" >&2; exit 1; }
DISPLAY=":$display_number" env -u XAUTHORITY dbus-run-session -- bash "$(dirname "$0")/appimage-catalog-smoke.sh" "$@"
