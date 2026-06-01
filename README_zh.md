# keyboard_dances

`keyboard_dances` 是一个 Linux 桌面应用，用来给键盘按下和释放事件配置不同音效。当前版本基于 Tauri 2 + Rust，提供应用窗口、system tray、配置文件和 profile 切换。

当前功能范围：

- 为所有按键统一配置“按下”音效和“释放”音效。
- 在应用 UI 中选择音频文件、保存配置、测试播放。
- 支持多个 profile，切换 profile 等于切换一组按下 / 释放音效，并可复制或删除 profile。
- 后台监听键盘事件，并在按下 / 释放时播放对应声音。
- 当前只面向 Linux / NixOS 开发环境，优先支持 AppImage 打包。

## Nix 开发环境

推荐从仓库根目录进入开发环境：

```bash
cd /home/zerone/projects/keyboard_dances
nix develop path:.
```

使用 `path:.` 的原因是：如果 `flake.nix` 是本地新增文件但还没有被 Git 跟踪，直接执行 `nix develop` 可能会因为 flake 输入未被纳入 Git tree 而失败。

如果 flake 文件已经被 Git 跟踪，也可以直接执行：

```bash
nix develop
```

兼容入口：

```bash
nix-shell
```

进入 shell 后会设置 Tauri / WebKit / GTK / libinput / ALSA 等运行和编译依赖。当前前端是静态文件，不需要 `npm install`，也不需要启动前端 dev server。

针对 niri / Wayland 环境，dev shell 默认设置：

```text
WEBKIT_DISABLE_COMPOSITING_MODE=1
WEBKIT_DISABLE_DMABUF_RENDERER=1
GDK_BACKEND=x11
```

`GDK_BACKEND=x11` 会让 GTK / WebKitGTK 通过 Xwayland 运行，用于规避部分 NixOS + Wayland 下 WebKitGTK 渲染异常。如果这时报无法打开 display，检查 niri 会话是否已经安装并启用 `xwayland-satellite`；如果你的 niri 会话没有 Xwayland，可以临时覆盖为 `GDK_BACKEND=wayland cargo tauri dev`。

## 开发运行

进入 Nix shell 后，从仓库根目录运行：

```bash
cargo tauri dev
```

当前项目没有前端构建步骤，Tauri dev 会根据 `src-tauri/tauri.conf.json` 中的 `frontendDist` 加载 `ui/` 目录里的静态文件。

如果只是想直接运行 Rust 应用，也可以使用：

```bash
cargo run --manifest-path src-tauri/Cargo.toml
```

开发 UI 样式时优先使用 `cargo tauri dev`，这样静态资源加载路径和最终 Tauri 应用更一致。

## 手动测试流程

启动应用后按下面顺序检查：

1. 应用窗口是否正常打开，UI 是否完整显示。
2. system tray 是否出现图标，托盘菜单是否可用。
3. 在 UI 中选择 Press 音频和 Release 音频。
4. 保存配置后，点击测试按钮确认按下 / 释放音效都能播放。
5. 新建、复制或切换 profile，确认配置随 profile 切换。
6. 实际按键按下和释放，确认后台监听能触发对应音效。

配置文件默认写入：

```text
~/.config/keyboard-dances/app.toml
~/.config/keyboard-dances/profiles/*.toml
```

默认示例声音默认写入：

```text
~/.local/share/keyboard-dances/sounds/
```

## Linux 输入和 Wayland 权限

应用通过 Linux 输入设备 `/dev/input/event*` 监听键盘事件。这个权限和桌面显示协议是两件事：Wayland、X11、niri 或 Xwayland 不会自动授予全局键盘事件读取权限。因此可能出现 UI 正常、测试按钮有声音，但实际按键没有声音的情况，原因通常是当前用户不能读取输入设备。

快速检查：

```bash
groups
ls -l /dev/input/event*
test -r /dev/input/event0
```

`test -r` 里的 event 设备路径要换成你机器上实际的键盘设备。如果读取检查失败，可以选择：

- 把当前用户加入 `input` 组。
- 为需要读取的键盘设备添加更精确的 udev 规则。

在 NixOS 中，用户组方式通常类似：

```nix
users.users.<your-user>.extraGroups = [ "input" ];
```

应用系统配置后，需要注销并重新登录，让桌面会话拿到新的用户组：

```bash
sudo nixos-rebuild switch
```

`input` 组可以读取设备的原始输入事件，权限范围比较大。如果希望限制得更细，优先考虑针对键盘设备写专门的 udev 规则。

针对 niri / Wayland 渲染，当前 dev shell 使用 `GDK_BACKEND=x11`，所以应用窗口需要 Xwayland 支持，例如 `xwayland-satellite`。这个显示设置只影响 Tauri / WebKit 窗口渲染；实际键盘监听仍然取决于 `/dev/input/event*` 的读取权限。

## AppImage 打包

当前只优先支持 AppImage。确认应用手动测试正常后，再进入 Nix shell 执行 NixOS 包装脚本：

```bash
cd /home/zerone/projects/keyboard_dances
nix develop path:.
scripts/build-appimage-nixos.sh
```

在 NixOS 上，直接执行 `cargo tauri build --bundles appimage` 可能会在
`failed to run linuxdeploy` 失败，因为 Tauri 会使用它缓存的 linuxdeploy
AppImage。包装脚本仍然使用 Tauri 构建二进制并生成 AppDir，然后改用
Nixpkgs 的 `linuxdeploy` 完成 AppImage。

AppImage 产物会写到：

```text
target/release/bundle/appimage/
```

后续等 AppImage 稳定后，再考虑 deb / rpm 等其他发行格式。

## 项目结构

```text
src-tauri/
├── src/
│   ├── main.rs      # Tauri 应用入口、命令、system tray
│   ├── audio/       # 音频加载与播放
│   ├── input/       # Linux 输入事件监听
│   ├── config.rs    # 应用配置和 profile 读写
│   └── runtime.rs   # 后台运行状态
├── tauri.conf.json  # Tauri 配置与 AppImage bundle 配置
└── build.rs         # 构建期生成默认图标

ui/
├── index.html
├── main.js
└── styles.css

nix/
└── dev-shell.nix
```

核心依赖：Tauri 2、rodio、symphonia、input、directories、rfd、toml。

## 已知限制

- 当前只支持 Linux。
- 当前只支持所有按键共享一组按下 / 释放音效，暂不支持单个按键独立配置。
- 实际键盘监听依赖 `/dev/input/event*` 权限。
- AppImage 是当前优先打包目标，其他发行格式后续再支持。

## License

Released under the MIT License.
