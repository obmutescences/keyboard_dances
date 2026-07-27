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

search_paths=()
for path_var in LD_LIBRARY_PATH NIX_LD_LIBRARY_PATH; do
  raw_paths="${!path_var-}"
  if [ -n "$raw_paths" ]; then
    IFS=: read -ra paths <<< "$raw_paths"
    for p in "${paths[@]}"; do
      [ -n "$p" ] || continue
      search_paths+=("$p")
    done
  fi
done

if [ -d /run/current-system/sw/lib ]; then
  search_paths+=("/run/current-system/sw/lib")
fi

echo "Fixing missing libraries in AppDir (NixOS workaround)..."
# 1. 补充 linuxdeploy 遗漏的核心 .so
libdir="$appdir/usr/lib"
for lib in \
  libwayland-client.so.0 \
  libX11.so.6 \
  libxcb.so.1 \
  libX11-xcb.so.1 \
  libasound.so.2 \
  libpipewire-0.3.so.0 \
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
  libGLdispatch.so.0 \
  libstdc++.so.6 \
; do
  if [ ! -f "$libdir/$lib" ]; then
    found=""
    for p in "${search_paths[@]}"; do
      candidate="$p/$lib"
      if [ -e "$candidate" ]; then
        found="$candidate"
        break
      fi
    done
    if [ -n "$found" ]; then
      cp -L "$found" "$libdir/$lib"
      echo "  + added $lib"
    else
      echo "  ! $lib not found in search paths"
    fi
  fi
done

# 2. Bundle ALSA's complete configuration tree. libasound embeds its Nix store
# data directory at build time, so config files must be made relocatable too.
alsa_config_src=""
for p in "${search_paths[@]}"; do
  candidate="$(dirname "$p")/share/alsa"
  if [ -f "$candidate/alsa.conf" ]; then
    alsa_config_src="$(realpath "$candidate")"
    break
  fi
done

if [ -z "$alsa_config_src" ]; then
  echo "  ! ALSA configuration tree not found in search paths" >&2
  exit 1
fi

alsa_config_dir="$appdir/usr/share/alsa"
mkdir -p "$alsa_config_dir"
cp -LR "$alsa_config_src/." "$alsa_config_dir/"
echo "  + ALSA configuration tree"

# 3. 补充 PipeWire ALSA 插件（让 ALSA 接口能通过 PipeWire 输出）
alsa_lib_dir="$libdir/alsa-lib"
mkdir -p "$alsa_lib_dir"
# 从已知库路径中找到 pipewire 的 alsa-lib 目录
alsa_plugin_src=""
for p in "${search_paths[@]}"; do
  for candidate in \
    "$p/alsa-lib" \
    "$(dirname "$p")/lib/alsa-lib" \
    "$(dirname "$p")/alsa-lib" \
  ; do
    if [ -f "$candidate/libasound_module_pcm_pipewire.so" ]; then
      alsa_plugin_src="$(realpath "$candidate")"
      break 2
    fi
  done
done
if [ -n "$alsa_plugin_src" ]; then
  echo "  Copying ALSA pipewire plugins from $alsa_plugin_src"
  for f in "$alsa_plugin_src/"*.so; do
    cp -L "$f" "$alsa_lib_dir/"
    echo "  + alsa-lib/$(basename $f)"
  done
  # 复制 ALSA pipewire 配置文件
  alsa_conf_src="$(dirname "$alsa_plugin_src")/../share/alsa/alsa.conf.d"
  if [ -d "$alsa_conf_src" ]; then
    mkdir -p "$alsa_config_dir/alsa.conf.d"
    for f in "$alsa_conf_src/"*.conf; do
      cp -L "$f" "$alsa_config_dir/alsa.conf.d/"
      echo "  + alsa/$(basename $f)"
    done
  fi
else
  echo "  ! ALSA pipewire plugin not found in search paths"
fi

# WebKitGTK launches helper processes from libexec. linuxdeploy copies its shared
# libraries but not these executables, leaving its embedded Nix store path behind.
webkit_libexec_src=""
webkit_lib_src=""
for p in "${search_paths[@]}"; do
  candidate="$(dirname "$p")/libexec/webkit2gtk-4.1"
  if [ -x "$candidate/WebKitNetworkProcess" ]; then
    webkit_libexec_src="$candidate"
    webkit_lib_src="$(dirname "$p")/lib/webkit2gtk-4.1"
    break
  fi
done

if [ -z "$webkit_libexec_src" ]; then
  echo "  ! WebKitGTK helper processes not found in search paths" >&2
  exit 1
fi

webkit_exec_dir="$appdir/usr/libexec/webkit2gtk-4.1"
mkdir -p "$webkit_exec_dir"
for process in WebKitGPUProcess WebKitNetworkProcess WebKitWebProcess; do
  cp -L "$webkit_libexec_src/$process" "$webkit_exec_dir/$process"
  chmod +x "$webkit_exec_dir/$process"
  echo "  + libexec/webkit2gtk-4.1/$process"
done

# Keep the WebKit injected bundle beside the copied helpers as well.
if [ -d "$webkit_lib_src/injected-bundle" ]; then
  mkdir -p "$libdir/webkit2gtk-4.1/injected-bundle"
  cp -LR "$webkit_lib_src/injected-bundle/." "$libdir/webkit2gtk-4.1/injected-bundle/"
  echo "  + WebKit injected bundle"
fi

# Recursively bundle the helper-process dependency closure and rewrite its RPATH.
linuxdeploy --verbosity 3 --appdir "$appdir" --deploy-deps-only "$webkit_exec_dir"

# Nix store files retain read-only modes when copied by linuxdeploy. Make the
# AppDir writable before its ELF metadata is normalized below.
chmod -R u+w "$appdir/usr"

# WebKitGTK 2.52 embeds its libexec directory in libwebkit2gtk. The
# WEBKIT_EXEC_PATH variable is not read by this version, so rewrite that path
# in-place to a procfs path resolved from the AppImage working directory. Keeping
# the replacement the same size preserves the ELF layout and avoids corrupting
# offsets in the binary.
#
# AppRun.wrapped enters the AppImage's usr directory before executing the main
# binary, so /proc/self/cwd/libexec reaches the bundled helpers regardless of
# the caller's original working directory.
webkit_exec_relative="/proc/self/cwd/libexec/webkit2gtk-4.1"
webkit_patch_count=0
while IFS= read -r -d '' webkit_so; do
  if perl -0777 -ne 'if (m{/nix/store/[^/\0]*-webkitgtk-[^/\0]*/libexec/webkit2gtk-4\.1}) { $found = 1 } END { exit($found ? 0 : 1) }' "$webkit_so"; then
    WEBKIT_EXEC_RELATIVE="$webkit_exec_relative" perl -0777 -pi -e 'my $replacement = $ENV{WEBKIT_EXEC_RELATIVE}; s{(/nix/store/[^/\0]*-webkitgtk-[^/\0]*/libexec/webkit2gtk-4\.1)}{my $old = $1; die "WebKit path replacement is longer than the original" if length($replacement) > length($old); $replacement . "\0" x (length($old) - length($replacement))}gex' "$webkit_so"
    webkit_patch_count=$((webkit_patch_count + 1))
    echo "  + made WebKitGTK libexec path relocatable in $(basename "$webkit_so")"
  fi
done < <(find "$libdir" -type f -name 'libwebkit2gtk-4.1.so*' -print0)

if [ "$webkit_patch_count" -eq 0 ]; then
  echo "  ! WebKitGTK main library with embedded libexec path was not found" >&2
  exit 1
fi

# Keep the AppImage root as the working directory for resources; helper lookup
# uses /proc/self/cwd after AppRun.wrapped enters the AppImage's usr directory.
cat > "$appdir/AppRun" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

this_dir="$(readlink -f "$(dirname "$0")")"
cd "$this_dir"

source "$this_dir/apprun-hooks/linuxdeploy-plugin-gtk.sh"

export ALSA_CONFIG_DIR="$this_dir/usr/share/alsa"
export ALSA_CONFIG_PATH="$ALSA_CONFIG_DIR/alsa.conf"
export ALSA_PLUGIN_DIR="$this_dir/usr/lib/alsa-lib"
export WEBKIT_INJECTED_BUNDLE_PATH="$this_dir/usr/lib/webkit2gtk-4.1/injected-bundle"
export LD_LIBRARY_PATH="$this_dir/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"

exec "$this_dir/AppRun.wrapped" "$@"
EOF
chmod +x "$appdir/AppRun"

# Rewrite every bundled ELF after all copying is complete. This removes Nix store
# RUNPATHs from recursively deployed WebKit dependencies as well as the main app.
while IFS= read -r -d '' elf; do
  if patchelf --print-needed "$elf" >/dev/null 2>&1; then
    elf_dir="$(dirname "$elf")"
    relative_libdir="$(realpath --relative-to="$elf_dir" "$libdir")"
    if [ "$relative_libdir" = "." ]; then
      elf_rpath='$ORIGIN'
    else
      elf_rpath="\$ORIGIN/$relative_libdir"
    fi
    patchelf --set-rpath "$elf_rpath" "$elf"
  fi
done < <(find "$appdir/usr" -type f -print0)

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
