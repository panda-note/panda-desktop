#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
target="x86_64-unknown-linux-gnu"
target_dir="${CARGO_TARGET_DIR:-/tmp/panda-target}"
app_dir="$(mktemp -d "${TMPDIR:-/tmp}/panda-appdir.XXXXXX")"
trap 'rm -rf -- "$app_dir"' EXIT
tool_dir="${PANDA_APPIMAGE_TOOL_DIR:-${TMPDIR:-/tmp}/panda-appimage-tools}"
output_dir="$repo_root/dist/linux"
desktop_file="$repo_root/crates/panda/resources/linux/panda.desktop"
icon_file="$repo_root/crates/panda/resources/linux/panda.png"
version="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$repo_root/crates/panda/Cargo.toml" | head -n 1 | tr -d '\r')"

if [[ -z "$version" ]]; then
  echo "Could not read Panda's version from crates/panda/Cargo.toml" >&2
  exit 1
fi

if [[ ! -f "$icon_file" ]]; then
  echo "Missing Linux app icon: $icon_file" >&2
  exit 1
fi

mkdir -p "$tool_dir" "$output_dir" \
  "$app_dir/usr/bin" \
  "$app_dir/usr/share/applications" \
  "$app_dir/usr/share/icons/hicolor/256x256/apps"

linuxdeploy="$tool_dir/linuxdeploy-x86_64.AppImage"
if [[ ! -x "$linuxdeploy" ]]; then
  curl --fail --location --retry 3 \
    https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage \
    --output "$linuxdeploy"
  chmod +x "$linuxdeploy"
fi

appimagetool="$tool_dir/appimagetool-x86_64.AppImage"
if [[ ! -x "$appimagetool" ]]; then
  curl --fail --location --retry 3 \
    https://github.com/AppImage/appimagetool/releases/download/continuous/appimagetool-x86_64.AppImage \
    --output "$appimagetool"
  chmod +x "$appimagetool"
fi

export CARGO_TARGET_DIR="$target_dir"
cd "$repo_root"
binary="$target_dir/$target/release-size/panda"
if [[ "${PANDA_SKIP_BUILD:-0}" != 1 ]]; then
  cargo build --locked -p panda --profile release-size --target "$target"
elif [[ ! -x "$binary" ]]; then
  echo "PANDA_SKIP_BUILD=1 was set, but no built Panda binary exists at $binary" >&2
  exit 1
fi

install -m 755 "$binary" "$app_dir/usr/bin/panda"
install -m 644 "$desktop_file" "$app_dir/usr/share/applications/panda.desktop"
install -m 644 "$icon_file" "$app_dir/usr/share/icons/hicolor/256x256/apps/panda.png"

export VERSION="$version"
export ARCH=x86_64
export APPIMAGE_EXTRACT_AND_RUN=1
"$linuxdeploy" \
  --appdir "$app_dir" \
  --executable "$app_dir/usr/bin/panda" \
  --desktop-file "$app_dir/usr/share/applications/panda.desktop" \
  --icon-file "$app_dir/usr/share/icons/hicolor/256x256/apps/panda.png"

"$appimagetool" "$app_dir" "$output_dir/PandaNote-$version-linux-x86_64.AppImage"

echo "AppImage created: $output_dir/PandaNote-$version-linux-x86_64.AppImage"
