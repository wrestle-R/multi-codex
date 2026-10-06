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
  Linux)
    [[ "$(uname -m)" == "x86_64" ]] || { printf 'Linux packages currently require x86_64.\n' >&2; exit 1; }
    suffix=".AppImage"
    ;;
  Darwin) suffix=".dmg" ;;
  *) printf 'Linux x86_64 and supported macOS installations are required.\n' >&2; exit 1 ;;
esac

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/multi-codex-update.XXXXXX")
mount_dir=""
cleanup() {
  if [[ -n "$mount_dir" ]]; then
    hdiutil detach "$mount_dir" >/dev/null 2>&1 || true
  fi
  if [[ -n "${staged:-}" ]]; then
    rm -rf "$staged"
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
  major=${version%%.*}
  minor_patch=${version#*.}
  minor=${minor_patch%%.*}
  if (( major > 1 || (major == 1 && minor >= 3) )); then
    [[ "$(uname -m)" == "arm64" ]] || { printf 'Multi Codex 1.3+ supports Apple Silicon Macs. Your existing installation has not been changed.\n' >&2; exit 1; }
    macos_major=$(sw_vers -productVersion)
    macos_major=${macos_major%%.*}
    (( macos_major >= 26 )) || { printf 'Multi Codex 1.3+ requires macOS 26 or newer. Your existing installation has not been changed.\n' >&2; exit 1; }
    asset_name="Multi.Codex_${version}_aarch64.dmg"
  else
    asset_name="Multi.Codex_${version}_universal.dmg"
  fi
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
  # The bundled repository license must be accepted when mounting the DMG
  # without a terminal. Feed one answer rather than an unbounded yes process.
  hdiutil attach -nobrowse -readonly -mountpoint "$mount_dir" "$work_dir/$asset_name" <<< 'Y' >/dev/null
  app_source="$mount_dir/Multi Codex.app"
  if [[ ! -d "$app_source" ]]; then
    hdiutil detach "$mount_dir" >/dev/null
    printf 'The verified DMG does not contain Multi Codex.app.\n' >&2
    exit 1
  fi
  mkdir -p "$HOME/Applications"
  destination="$HOME/Applications/Multi Codex.app"
  staged="$HOME/Applications/.Multi Codex.app.new.$$.app"
  rm -rf "$staged"
  ditto "$app_source" "$staged"
  hdiutil detach "$mount_dir" >/dev/null
  mount_dir=""
  executable=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleExecutable' "$staged/Contents/Info.plist")
  if [[ -z "$executable" || "$executable" == */* || ! -x "$staged/Contents/MacOS/$executable" ]]; then
    printf 'The verified DMG does not contain a valid app executable. Your existing installation has not been changed.\n' >&2
    exit 1
  fi
  if (( major > 1 || (major == 1 && minor >= 3) )); then
    signature_details=$(codesign --display --verbose=2 "$staged" 2>&1 || true)
    allow_unsigned_mac=0
    if [[ "$signature_details" != *"Authority="* ]]; then
      if [[ "${MULTI_CODEX_ALLOW_UNSIGNED_MAC:-0}" != "1" ]]; then
        printf 'This Mac build has no Apple Developer ID signature. Your existing installation has not been changed. Use the unsigned-release Mac command in the README.\n' >&2
        exit 1
      fi
      allow_unsigned_mac=1
    fi
    if ! codesign --verify --deep --strict "$staged"; then
      if (( ! allow_unsigned_mac )); then
        printf 'Mac signature verification failed. Your existing installation has not been changed. For this unsigned release, use the Mac command in the README.\n' >&2
        exit 1
      fi
      # Apple Silicon requires a code signature. Sign only the verified staged
      # unsigned app locally; an identified developer signature is never replaced.
      codesign --force --deep --sign - "$staged"
      codesign --verify --deep --strict "$staged"
    fi
    if ! spctl --assess --type execute --verbose "$staged"; then
      if (( ! allow_unsigned_mac )); then
        printf 'Mac approval verification failed. Your existing installation has not been changed.\n' >&2
        exit 1
      fi
      printf 'Installing the checksum-verified app without Apple notarization. If macOS blocks first launch, use System Settings > Privacy & Security > Open Anyway for Multi Codex.\n'
    fi
  fi
  backup="$HOME/Applications/.Multi Codex.app.previous.$$"
  if [[ -e "$destination" ]]; then mv "$destination" "$backup"; fi
  if ! mv "$staged" "$destination"; then
    [[ ! -e "$backup" ]] || mv "$backup" "$destination"
    printf 'Could not replace Multi Codex; the previous app was restored.\n' >&2
    exit 1
  fi
  rm -rf "$backup"
  printf 'Installed %s at %s. Your Multi Codex profile data was not modified.\n' "$tag" "$destination"
  if [[ "${MULTI_CODEX_NO_LAUNCH:-0}" != "1" ]]; then open "$destination"; fi
fi
