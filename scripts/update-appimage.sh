#!/usr/bin/env bash
set -euo pipefail

repo="wrestle-R/multi-codex"
api_url="https://api.github.com/repos/${repo}/releases/latest"

for command in curl jq sha256sum; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'Multi Codex updater requires %s.\n' "$command" >&2
    exit 1
  fi
done

work_dir=$(mktemp -d "${TMPDIR:-/tmp}/multi-codex-update.XXXXXX")
trap 'find "$work_dir" -depth -delete' EXIT

release_json="$work_dir/release.json"
curl --fail --silent --show-error --location "$api_url" --output "$release_json"

tag=$(jq -er '.tag_name' "$release_json")
appimage_url=$(jq -er '[.assets[] | select(.name | endswith(".AppImage"))] | if length == 1 then .[0].browser_download_url else error("expected one AppImage") end' "$release_json")
checksums_url=$(jq -er '[.assets[] | select(.name == "SHA256SUMS")] | if length == 1 then .[0].browser_download_url else error("missing SHA256SUMS") end' "$release_json")
appimage_name=${appimage_url##*/}

printf 'Downloading Multi Codex %s...\n' "$tag"
curl --fail --silent --show-error --location "$appimage_url" --output "$work_dir/$appimage_name"
curl --fail --silent --show-error --location "$checksums_url" --output "$work_dir/SHA256SUMS"

checksum_line=$(awk -v name="$appimage_name" '$2 == name { print; found = 1 } END { if (!found) exit 1 }' "$work_dir/SHA256SUMS")
printf '%s\n' "$checksum_line" > "$work_dir/APPIMAGE_SHA256"
(cd "$work_dir" && sha256sum --check APPIMAGE_SHA256)

chmod 0755 "$work_dir/$appimage_name"
printf 'Verified %s. If prompted, choose Install to refresh the launcher and icons.\n' "$tag"
APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}" "$work_dir/$appimage_name"
