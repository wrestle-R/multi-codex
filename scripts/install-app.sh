#!/usr/bin/env bash
set -euo pipefail

repo="wrestle-R/multi-codex"
latest_url="https://github.com/${repo}/releases/latest"

for command in curl; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'Multi Codex updater requires %s.\n' "$command" >&2
    exit 1
  fi
done

sha256_file() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | awk '{print $1}'
  else
    printf 'Multi Codex updater requires sha256sum or shasum.\n' >&2
    exit 1
  fi
}

case "$(uname -s)" in
  Linux) suffix=".AppImage" ;;
  Darwin) suffix=".dmg" ;;
  *) printf 'Only Arch Linux and macOS are supported.\n' >&2; exit 1 ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/multi-codex-update.XXXXXX")
mount_dir=""
cleanup() {
  if [[ -n "$mount_dir" ]]; then
    hdiutil detach "$mount_dir" >/dev/null 2>&1 || true
  fi
  rm -rf "$work_dir"
}
trap cleanup EXIT

resolved_url=$(curl --fail --silent --show-error --location --retry 4 --retry-all-errors --retry-delay 2 --output /dev/null --write-out '%{url_effective}' "$latest_url")
tag=${resolved_url##*/}
if [[ ! "$tag" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  printf 'Could not determine the latest Multi Codex release.\n' >&2
  exit 1
fi
version=${tag#v}
if [[ "$suffix" == ".AppImage" ]]; then
  asset_name="Multi.Codex_${version}_amd64.AppImage"
else
  asset_name="Multi.Codex_${version}_universal.dmg"
fi
release_url="https://github.com/${repo}/releases/download/${tag}"
asset_url="$release_url/$asset_name"
checksums_url="$release_url/SHA256SUMS"

printf 'Downloading Multi Codex %s...\n' "$tag"
curl --fail --silent --show-error --location --retry 4 --retry-all-errors --retry-delay 2 "$asset_url" --output "$work_dir/$asset_name"
curl --fail --silent --show-error --location --retry 4 --retry-all-errors --retry-delay 2 "$checksums_url" --output "$work_dir/SHA256SUMS"

expected_checksum=$(awk -v name="$asset_name" '$2 == name { print $1; found = 1 } END { if (!found) exit 1 }' "$work_dir/SHA256SUMS")
actual_checksum=$(sha256_file "$work_dir/$asset_name")
if [[ "$actual_checksum" != "$expected_checksum" ]]; then
  printf 'Checksum verification failed for %s.\n' "$asset_name" >&2
  exit 1
fi
printf '%s: OK\n' "$asset_name"

if [[ "$suffix" == ".AppImage" ]]; then
  legacy_destination="$HOME/.local/bin/multi-codex.AppImage"
  applications_destination="$HOME/Applications/Multi.Codex.AppImage"
  if [[ -e "$legacy_destination" ]]; then
    destination="$legacy_destination"
  elif [[ -e "$applications_destination" ]]; then
    destination="$applications_destination"
  else
    destination="$legacy_destination"
  fi
  mkdir -p "$(dirname "$destination")"
  staged="$(dirname "$destination")/.Multi.Codex.AppImage.new.$$"
  install -m 0755 "$work_dir/$asset_name" "$staged"
  mv -f "$staged" "$destination"
  printf 'Installed %s at %s. Your Multi Codex profile data was not modified.\n' "$tag" "$destination"
  if [[ "${MULTI_CODEX_NO_LAUNCH:-0}" != "1" ]]; then
    APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}" "$destination"
  fi
else
  mount_dir="$work_dir/dmg"
  mkdir "$mount_dir"
  hdiutil attach -nobrowse -readonly -mountpoint "$mount_dir" "$work_dir/$asset_name" >/dev/null
  app_source="$mount_dir/Multi Codex.app"
  if [[ ! -d "$app_source" ]]; then
    hdiutil detach "$mount_dir" >/dev/null
    printf 'The verified DMG does not contain Multi Codex.app.\n' >&2
    exit 1
  fi
  mkdir -p "$HOME/Applications"
  destination="$HOME/Applications/Multi Codex.app"
  staged="$HOME/Applications/.Multi Codex.app.new.$$"
  rm -rf "$staged"
  ditto "$app_source" "$staged"
  hdiutil detach "$mount_dir" >/dev/null
  mount_dir=""
  rm -rf "$destination"
  mv "$staged" "$destination"
  executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$destination/Contents/Info.plist")
  chmod +x "$destination/Contents/MacOS/$executable"
  printf 'Installed %s at %s.\n' "$tag" "$destination"
  open "$destination"
fi
