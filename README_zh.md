# keyboard_dances

键盘敲击时播放音效的 Linux 后台工具，使用 Rust 构建。程序通过 libinput 监听键盘事件，在按下与抬起时分别播放自定义音频。

## 功能特性

- 基于 libinput 监听 `seat0` 键盘事件，适用于 Wayland 与 X11（前提是 compositor 使用 libinput）。
- 按键按下 / 释放分别绑定音效，支持 WAV、OGG / Vorbis。
- 启动时自动播放一次测试音效确认加载成功，随后常驻后台运行。
- 运行过程中会打印简洁日志（音频加载、设备插拔、键盘事件）。

## 快速开始

1. 安装 Rust 稳定版（2021 edition）。
2. 安装系统依赖：`libinput`、`libudev`、`alsa-lib`、`libxkbcommon`、`pkg-config`。
   - Debian / Ubuntu：
     ```bash
     sudo apt install libinput-dev libudev-dev libasound2-dev libxkbcommon-dev pkg-config
     ```
   - 或进入项目提供的 Nix 开发环境：
     ```bash
     nix-shell
     ```
3. 准备两段音频文件（可使用仓库里的 `ff-0.wav` / `ff-1.wav`）：
   - 第一个参数：按键按下时播放的音效。
   - 第二个参数：按键抬起时播放的音效。
4. 以拥有 `/dev/input` 访问权限的身份运行（普通用户可以加入 `input` 组或使用 `sudo`）：
   ```bash
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav
   ```

程序启动后会播放一次按下 / 抬起音效验证加载情况，然后进入监听循环。`Ctrl+C` 终止进程。

## 构建与运行

```bash
# Debug 构建
cargo build

# Release 构建（推荐）
cargo build --release

# 运行
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND>
```

CLI 会校验参数指向的文件是否存在。推荐提供绝对路径或相对于当前目录的路径。

## 使用说明

- 支持音频格式：WAV（PCM）、OGG / Vorbis，通过 `symphonia` 解码。
- 多键几乎同时触发时音效会混合播放，依赖 rodio 的 `Sink` 自动混音。
- 默认监听 `seat0`，暂未暴露自定义 seat 或过滤设备的配置。
- 日志中会显示音频文件信息、设备接入 / 移除及按键事件。

## 常见问题

- **缺少系统库**：若构建时报 `alsa` 或 `libudev` not found，请确认相应开发包已安装，或在 `nix-shell` 中构建。
- **无按键事件**：确认 compositor 使用 libinput，并确保进程具有读取 `/dev/input/event*` 的权限。
- **权限被拒绝**：加入 `input` 组或使用 `sudo`，也可以通过 udev 规则放宽权限。
- **听不到声音**：检查系统音量；使用 `aplay path/to/file.wav` 测试音频文件是否可播放。

## 项目结构

```
src/
├── main.rs      # CLI 解析、音频加载、事件循环启动
├── audio/       # 音频模块（加载 + 播放）
│   └── mod.rs
└── input/       # 输入模块（libinput 监听）
    └── mod.rs
```

核心依赖：`rodio`、`symphonia`、`input`（libinput 绑定）、`clap`、`anyhow`。

## 已知限制

- 仅在 Linux 下工作（`cfg(target_os = "linux")`）。
- 需要 libinput 支撑的物理键盘；虚拟键盘或远程输入暂未验证。
- 项目尚无自动化测试，请在目标桌面环境中手动验证。

## 许可证

本项目遵循 MIT 许可证。
