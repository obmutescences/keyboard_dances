#!/usr/bin/env bash
set -euo pipefail

script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repo_root="$(CDPATH= cd -- "$script_dir/.." && pwd)"

target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"
case "$target_dir" in
  /*) ;;
  *) target_dir="$repo_root/$target_dir" ;;
esac

appimage_dir="$target_dir/release/bundle/appimage"
appdir="$appimage_dir/Keyboard Dances.AppDir"
tauri_cache="${TAURI_BUNDLER_CACHE:-$HOME/.cache/tauri}"
gtk_plugin="$tauri_cache/linuxdeploy-plugin-gtk.sh"
appimage_plugin="$tauri_cache/linuxdeploy-plugin-appimage.AppImage"

cd "$repo_root"

echo "Building release binary without bundling..."
(
  cd "$repo_root/src-tauri"
  cargo tauri build --no-bundle "$@"
)

if [ -d "$target_dir/release/bundle" ]; then
  chmod -R u+w "$target_dir/release/bundle" 2>/dev/null || true
fi

echo "Preparing AppDir through Tauri bundler..."
set +e
(
  cd "$repo_root/src-tauri"
  cargo tauri bundle --bundles appimage
)
bundle_status=$?
set -e

if [ "$bundle_status" -ne 0 ]; then
  echo "Tauri AppImage bundling failed on linuxdeploy; continuing with NixOS linuxdeploy workaround."
fi

if [ ! -d "$appdir" ]; then
  echo "AppDir was not created: $appdir" >&2
  exit "$bundle_status"
fi

if [ ! -x "$gtk_plugin" ]; then
  echo "Missing Tauri GTK plugin: $gtk_plugin" >&2
  exit 1
fi

if [ ! -x "$appimage_plugin" ]; then
  echo "Missing Tauri AppImage plugin: $appimage_plugin" >&2
  exit 1
fi

chmod -R u+w "$target_dir/release/bundle" 2>/dev/null || true

plugin_dir="$(mktemp -d)"
trap 'rm -rf "$plugin_dir"' EXIT
ln -s "$gtk_plugin" "$plugin_dir/linuxdeploy-plugin-gtk.sh"

echo "Running Nixpkgs linuxdeploy with the GTK plugin..."
PATH="$plugin_dir:$PATH" linuxdeploy --verbosity 3 --appdir "$appdir" --plugin gtk

echo "Fixing missing libraries in AppDir (NixOS workaround)..."
# linuxdeploy 在 NixOS 上依赖解析不完整，会遗漏一些核心库
# 这里从 LD_LIBRARY_PATH 中手动补充缺失的 .so
libdir="$appdir/usr/lib"
for lib in \
  libwayland-client.so.0 \
  libX11.so.6 \
  libxcb.so.1 \
  libX11-xcb.so.1 \
  libasound.so.2 \
  libfribidi.so.0 \
  libfontconfig.so.1 \
  libfreetype.so.6 \
  libharfbuzz.so.0 \
  libexpat.so.1 \
  libz.so.1 \
  libgpg-error.so.0 \
  libgbm.so.1 \
  libdrm.so.2 \
  libEGL.so.1 \
  libGLX.so.0 \
  libstdc++.so.6 \
; do
  if [ ! -f "$libdir/$lib" ]; then
    found=""
    IFS=: read -ra paths <<< "$LD_LIBRARY_PATH"
    for p in "${paths[@]}"; do
      candidate="$p/$lib"
      if [ -f "$candidate" ] && [ ! -L "$candidate" ]; then
        found="$candidate"
        break
      fi
    done
    if [ -n "$found" ]; then
      cp -L "$found" "$libdir/$lib"
      echo "  + added $lib"
    else
      echo "  ! $lib not found in LD_LIBRARY_PATH"
    fi
  fi
done
# 重新设置 RPATH，确保 AppDir 内的 lib 目录优先
appdir_lib="\$ORIGIN/../lib"
for binary in "$appdir/usr/bin/"*; do
  if [ -x "$binary" ] && [ ! -d "$binary" ]; then
    patchelf --set-rpath "$appdir_lib:$ORIGIN" "$binary" 2>/dev/null || true
  fi
done
# 对 usr/lib 下的 .so 也设置 RPATH
for so in "$libdir/"*.so*; do
  if [ -f "$so" ] && [ ! -L "$so" ]; then
    patchelf --set-rpath "$appdir_lib" "$so" 2>/dev/null || true
  fi
done

echo "Generating AppImage..."
APPIMAGE_EXTRACT_AND_RUN=1 "$appimage_plugin" --appdir "$appdir"

product_name="$(sed -n 's/.*"productName"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$repo_root/src-tauri/tauri.conf.json" | head -n 1)"
version="$(sed -n 's/.*"version"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' "$repo_root/src-tauri/tauri.conf.json" | head -n 1)"

case "$(uname -m)" in
  x86_64)
    appimage_arch="x86_64"
    bundle_arch="amd64"
    ;;
  aarch64)
    appimage_arch="aarch64"
    bundle_arch="arm64"
    ;;
  *)
    appimage_arch="$(uname -m)"
    bundle_arch="$(uname -m)"
    ;;
esac

generated_name="${product_name// /_}-$appimage_arch.AppImage"
generated_path="$repo_root/$generated_name"
final_path="$appimage_dir/${product_name}_${version}_${bundle_arch}.AppImage"

if [ ! -f "$generated_path" ]; then
  echo "Generated AppImage was not found: $generated_path" >&2
  exit 1
fi

mkdir -p "$appimage_dir"
mv -f "$generated_path" "$final_path"
chmod +x "$final_path"

echo "Built AppImage: $final_path"
