#!/usr/bin/env bash
set -euo pipefail

if [[ $# -ne 1 || ! -f "$1" ]]; then
  echo "Usage: $0 <candidate.AppImage>" >&2
  exit 2
fi

for command_name in convert firejail identify import xdotool; do
  command -v "$command_name" >/dev/null || {
    echo "Required smoke-test command is unavailable: $command_name" >&2
    exit 2
  }
done

appimage="$(readlink -f "$1")"
test_root="$(mktemp -d)"
app_pid=""

cleanup() {
  if [[ -n "$app_pid" ]] && kill -0 "$app_pid" 2>/dev/null; then
    kill "$app_pid" 2>/dev/null || true
    for _ in 1 2 3 4 5; do
      kill -0 "$app_pid" 2>/dev/null || break
      sleep 1
    done
    kill -9 "$app_pid" 2>/dev/null || true
  fi
  rm -rf "$test_root"
}
trap cleanup EXIT

chmod +x "$appimage"
mkdir -p "$test_root/config" "$test_root/data" "$test_root/cache" "$test_root/home"

env \
  HOME="$test_root/home" \
  WEBKIT_DISABLE_COMPOSITING_MODE=1 \
  WEBKIT_DISABLE_DMABUF_RENDERER=1 \
  XDG_CACHE_HOME="$test_root/cache" \
  XDG_CONFIG_HOME="$test_root/config" \
  XDG_DATA_HOME="$test_root/data" \
  firejail --quiet --noprofile --net=none --appimage "$appimage" \
  >"$test_root/appimage.log" 2>&1 &
app_pid=$!

# Match AppImageHub's minimum startup allowance before accepting a window.
sleep 10
for _ in $(seq 1 20); do
  if ! kill -0 "$app_pid" 2>/dev/null; then
    echo "AppImage exited before presenting a window" >&2
    sed -n '1,160p' "$test_root/appimage.log" >&2
    exit 1
  fi
  if timeout 5 xdotool search --onlyvisible --name '^Multi Codex$' >/dev/null 2>&1; then
    sleep 2
    break
  fi
  sleep 1
done

if ! kill -0 "$app_pid" 2>/dev/null; then
  echo "AppImage exited during the visible-window smoke test" >&2
  sed -n '1,160p' "$test_root/appimage.log" >&2
  exit 1
fi
window_id="$(timeout 5 xdotool search --onlyvisible --name '^Multi Codex$' 2>/dev/null | head -n 1 || true)"
if [[ -z "$window_id" ]]; then
  echo "AppImage did not present its visible Multi Codex window" >&2
  sed -n '1,160p' "$test_root/appimage.log" >&2
  exit 1
fi
if grep -Fq 'Unable to spawn a new child process' "$test_root/appimage.log"; then
  echo "AppImage failed to spawn a bundled WebKit helper" >&2
  sed -n '1,160p' "$test_root/appimage.log" >&2
  exit 1
fi

timeout 20 import -window "$window_id" "$test_root/screenshot.png"
if [[ "$(identify -format '%m' "$test_root/screenshot.png")" != "PNG" ]]; then
  echo "AppImage window smoke did not produce a PNG screenshot" >&2
  exit 1
fi
read -r top_pixels total_pixels < <(
  convert "$test_root/screenshot.png" -alpha off -depth 8 -format '%c' histogram:info:- \
    | sort -rn \
    | awk -v size="$(identify -format '%w %h' "$test_root/screenshot.png")" \
      'NR == 1 { top = $1 + 0 } END { split(size, dimensions, " "); print top, dimensions[1] * dimensions[2] }'
)
dominant_share="$(awk -v top="$top_pixels" -v total="$total_pixels" 'BEGIN { printf "%d", (total > 0 ? 100 * top / total : 100) }')"
if [[ "$dominant_share" -ge 95 ]]; then
  echo "AppImage window appears blank: ${dominant_share}% of pixels use one color" >&2
  exit 1
fi

echo "AppImage catalog launch smoke passed (${dominant_share}% dominant color)"
