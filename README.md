# keyboard_dances

`keyboard_dances` is a Rust background utility for Linux that plays custom audio when keys are pressed and released. It listens to keyboard events through libinput while the program keeps running in the background.

## Features

- Listens to `seat0` keyboard events via libinput and works on Wayland or X11 compositors that rely on libinput.
- Binds separate sounds to key press and key release events, supporting WAV and OGG/Vorbis formats.
- Plays a short startup test for both sounds to confirm they loaded, then continues running as a daemon.
- Emits lightweight logs during runtime (audio loading, device add/remove, key events).

## Quick Start

1. Install the Rust stable toolchain (2021 edition).
2. Install system dependencies: `libinput`, `libudev`, `alsa-lib`, `libxkbcommon`, and `pkg-config`.
   - Debian / Ubuntu:
     ```bash
     sudo apt install libinput-dev libudev-dev libasound2-dev libxkbcommon-dev pkg-config
     ```
   - Or use the bundled Nix development environment:
     ```bash
     nix-shell
     ```
3. Prepare two audio files (you can reuse the repository's `ff-0.wav` / `ff-1.wav`):
   - First argument: sound to play on key press.
   - Second argument: sound to play on key release.
4. Run with access to `/dev/input` (join the `input` group or run via `sudo`):
   ```bash
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav
   ```

The program plays both sounds once during startup to verify loading, then enters the event loop. Use `Ctrl+C` to stop it.

## Build & Run

```bash
# Debug build
cargo build

# Release build (recommended)
cargo build --release

# Run
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND>
```

The CLI validates that each argument points to an existing file. Provide absolute paths or paths relative to the current directory.

## Usage Notes

- Supported formats: WAV (PCM) and OGG/Vorbis, decoded via `symphonia`.
- When multiple keys are hit in quick succession the sounds overlap; rodio's `Sink` handles the mixing.
- The listener targets `seat0` by default; there is currently no configuration for custom seats or device filtering.
- Logs report audio metadata, device add/remove events, and keyboard activity.

## Troubleshooting

- **Missing system libraries**: If the build fails with `alsa` or `libudev` not found, install the appropriate development packages or build inside `nix-shell`.
- **No key events**: Ensure your compositor uses libinput and that the process can read `/dev/input/event*`.
- **Permission denied**: Join the `input` group, run via `sudo`, or relax access with a custom udev rule.
- **No audio output**: Check system volume; test the audio files with `aplay path/to/file.wav`.

## Project Structure

```
src/
├── main.rs      # CLI parsing, audio loading, event loop bootstrap
├── audio/       # Audio module (load + playback)
│   └── mod.rs
└── input/       # Input module (libinput listener)
    └── mod.rs
```

Core dependencies: `rodio`, `symphonia`, `input` (libinput bindings), `clap`, `anyhow`.

## Known Limitations

- Linux only (`cfg(target_os = "linux")`).
- Requires a physical keyboard managed by libinput; virtual keyboards or remote input setups are untested.
- No automated tests yet—verify behaviour in your target desktop environment.

## License

Released under the MIT License.
