#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
VERSION="$(node -e 'const fs=require("fs"); console.log(JSON.parse(fs.readFileSync("src-tauri/tauri.conf.json","utf8")).version)')"
EXPECTED_APPIMAGE="Multi.Codex_${VERSION}_amd64.AppImage"

if [[ ! "$VERSION" =~ ^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$ ]]; then
  echo "Invalid application version: $VERSION" >&2
  exit 1
fi
if [[ "$EXPECTED_APPIMAGE" != "Multi.Codex_${VERSION}_amd64.AppImage" ]]; then
  echo "Unexpected release AppImage name" >&2
  exit 1
fi
if ! grep -Fq 'LDAI_RUNTIME_FILE' "$ROOT_DIR/scripts/build-linux-release.sh"; then
  echo "Release builder does not provide an offline AppImage runtime fallback" >&2
  exit 1
fi

APPDIR="$TMP_DIR/Multi Codex.AppDir"
mkdir -p "$APPDIR/usr/bin" "$APPDIR/apprun-hooks" "$APPDIR/usr/share/applications" "$APPDIR/usr/share/icons/hicolor/128x128/apps"

cat > "$APPDIR/AppRun" <<'APP_RUN'
#!/usr/bin/env bash
source "$(dirname "$0")/apprun-hooks/linuxdeploy-plugin-gtk.sh"
exec "$(dirname "$0")/AppRun.wrapped" "$@"
APP_RUN
chmod +x "$APPDIR/AppRun"

cat > "$APPDIR/usr/bin/multi-codex-desktop" <<'APP_BIN'
#!/usr/bin/env bash
echo multi-codex
APP_BIN
chmod +x "$APPDIR/usr/bin/multi-codex-desktop"

cat > "$APPDIR/usr/share/applications/Multi Codex.desktop" <<'DESKTOP'
[Desktop Entry]
Name=Multi Codex
Exec=multi-codex-desktop
Icon=multi-codex-desktop
Type=Application
Categories=Utility;
DESKTOP
printf 'png' > "$APPDIR/usr/share/icons/hicolor/128x128/apps/multi-codex-desktop.png"

"$ROOT_DIR/scripts/build-linux-release.sh" --patch-appdir "$APPDIR"

grep -Fq 'LD_LIBRARY_PATH="/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"' "$APPDIR/AppRun"
if grep -Fq 'linuxdeploy-plugin-gtk.sh' "$APPDIR/AppRun"; then
  echo "AppRun still sources the linuxdeploy GTK hook" >&2
  exit 1
fi
"$APPDIR/AppRun" --version >/dev/null
test -f "$APPDIR/multi-codex-desktop.png"

echo "Release packaging checks passed"
