#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APPIMAGE_PLUGIN="${APPIMAGE_PLUGIN:-$HOME/.cache/tauri/linuxdeploy-plugin-appimage.AppImage}"
APPIMAGE_RUNTIME_CACHE="${APPIMAGE_RUNTIME_CACHE:-$HOME/.cache/tauri/runtime-x86_64}"
APPIMAGE_RUNTIME_SOURCE="${APPIMAGE_RUNTIME_SOURCE:-$HOME/.local/bin/multi-codex.AppImage}"

patch_appdir() {
  local appdir="$1"
  local app_bin="$appdir/usr/bin/multi-codex-desktop"
  local desktop_file root_desktop icon_name icon_path
  [[ -d "$appdir" ]] || { echo "AppDir not found: $appdir" >&2; exit 1; }
  [[ -x "$app_bin" ]] || { echo "Multi Codex binary not found: $app_bin" >&2; exit 1; }

  cat > "$appdir/AppRun" <<'APP_RUN'
#!/usr/bin/env bash
set -e
this_dir="$(readlink -f "$(dirname "$0")")"
export APPDIR="${APPDIR:-$this_dir}"
export LD_LIBRARY_PATH="/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$this_dir/usr/bin/multi-codex-desktop" "$@"
APP_RUN
  chmod +x "$appdir/AppRun"

  desktop_file="$(find "$appdir/usr/share/applications" -maxdepth 1 -type f -name '*.desktop' | sort | head -n 1)"
  if [[ -n "$desktop_file" ]]; then
    root_desktop="$appdir/$(basename "$desktop_file")"
    if [[ "$(readlink -f "$desktop_file")" != "$(readlink -f "$root_desktop" 2>/dev/null || true)" ]]; then
      cp -f "$desktop_file" "$root_desktop"
    fi
    icon_name="$(awk -F= '$1 == "Icon" { print $2; exit }' "$desktop_file")"
    if [[ -n "$icon_name" && "$icon_name" != /* ]]; then
      icon_path="$(find "$appdir/usr/share/icons" -type f \( -name "$icon_name.png" -o -name "$icon_name.svg" -o -name "$icon_name.xpm" \) | sort | head -n 1)"
      [[ -z "$icon_path" ]] || cp -f "$icon_path" "$appdir/$(basename "$icon_path")"
    fi
  fi
}

prepare_appimage_runtime() {
  local offset temporary_runtime
  if [[ -n "${LDAI_RUNTIME_FILE:-}" && -f "$LDAI_RUNTIME_FILE" ]]; then return; fi
  if [[ -f "$APPIMAGE_RUNTIME_CACHE" ]]; then export LDAI_RUNTIME_FILE="$APPIMAGE_RUNTIME_CACHE"; return; fi
  if [[ ! -x "$APPIMAGE_RUNTIME_SOURCE" ]]; then return; fi
  offset="$("$APPIMAGE_RUNTIME_SOURCE" --appimage-offset 2>/dev/null || true)"
  [[ "$offset" =~ ^[0-9]+$ && "$offset" -gt 0 ]] || return
  mkdir -p "$(dirname "$APPIMAGE_RUNTIME_CACHE")"
  temporary_runtime="${APPIMAGE_RUNTIME_CACHE}.tmp"
  head -c "$offset" "$APPIMAGE_RUNTIME_SOURCE" > "$temporary_runtime"
  chmod +x "$temporary_runtime"
  mv "$temporary_runtime" "$APPIMAGE_RUNTIME_CACHE"
  export LDAI_RUNTIME_FILE="$APPIMAGE_RUNTIME_CACHE"
}

if [[ "${1:-}" == "--patch-appdir" ]]; then
  [[ $# -eq 2 ]] || { echo "Expected an AppDir path" >&2; exit 1; }
  patch_appdir "$2"
  exit 0
fi
[[ $# -eq 0 ]] || { echo "Unexpected arguments" >&2; exit 1; }

cd "$ROOT_DIR"
version="$(node -e 'const fs=require("fs"); console.log(JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json","utf8")).version)')"
bundle_dir="$ROOT_DIR/src-tauri/target/release/bundle"
appimage_dir="$bundle_dir/appimage"
artifact_name="Multi.Codex_${version}_amd64.AppImage"

rm -rf "$bundle_dir"
npm test
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo test --manifest-path src-tauri/Cargo.toml
npm run check:secrets
npm run tauri -- build --bundles deb,rpm

set +e
npm run tauri -- build --bundles appimage
appimage_build_status=$?
set -e
if [[ "$appimage_build_status" -ne 0 ]]; then
  echo "Tauri AppImage finalization failed; attempting a patched repack"
fi

mapfile -t appdirs < <(find "$appimage_dir" -maxdepth 1 -type d -name '*.AppDir' | sort)
[[ "${#appdirs[@]}" -eq 1 ]] || { echo "Expected exactly one generated AppDir" >&2; exit 1; }
appdir="${appdirs[0]}"
patch_appdir "$appdir"

[[ -x "$APPIMAGE_PLUGIN" ]] || { echo "AppImage plugin is unavailable: $APPIMAGE_PLUGIN" >&2; exit 1; }
prepare_appimage_runtime

rm -f "$appimage_dir"/*.AppImage
(cd "$appimage_dir" && ARCH=x86_64 "$APPIMAGE_PLUGIN" --appdir="$appdir")
mapfile -t generated < <(find "$appimage_dir" -maxdepth 1 -type f -name '*.AppImage' | sort)
[[ "${#generated[@]}" -eq 1 ]] || { echo "Expected exactly one generated AppImage" >&2; exit 1; }
mv "${generated[0]}" "$appimage_dir/$artifact_name"

find "$bundle_dir" -type f \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' \) -print0 \
  | sort -z | xargs -0 sha256sum > "$ROOT_DIR/SHA256SUMS"

echo "Linux release artifacts:"
find "$bundle_dir" -type f \( -name '*.AppImage' -o -name '*.deb' -o -name '*.rpm' \) -print | sort
echo "$ROOT_DIR/SHA256SUMS"
