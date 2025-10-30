# keyboard_dances

`keyboard_dances` is a Rust background utility for Linux that plays custom audio when keys are pressed and released. It listens to keyboard events through libinput while the program keeps running in the background.

## Features

- Listens to keyboard events on Linux (Wayland compositor via libinput and wl_keyboard protocols)
- Binds separate sounds to key press and key release events, supporting WAV and OGG/Vorbis formats
- **Configurable volume control** - Adjust audio volume from 0.0 (silent) to any positive value
- **Sample rate control** - Modify pitch and playback speed with sample rate multiplier
- Plays a short startup test for both sounds to confirm they loaded, then continues running as a daemon
- Emits lightweight logs during runtime (audio loading, device add/remove, key events)

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
   # Basic usage
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav

   # With custom volume (0.0 = silent, 1.0 = normal, 2.0 = double volume)
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav --volume 0.5

   # With custom pitch/speed (1.0 = normal, 2.0 = double speed/high pitch, 0.5 = half speed/low pitch)
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav --sample-rate 1.5

   # Combined settings
   sudo cargo run --release -- ./ff-0.wav ./ff-1.wav --volume 0.3 --sample-rate 0.8
   ```

The program plays both sounds once during startup to verify loading, then enters the event loop. Use `Ctrl+C` to stop it.

## Build & Run

```bash
# Debug build
cargo build

# Release build (recommended)
cargo build --release

# Run with default settings
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND>

# Run with volume control
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND> --volume 0.5

# Run with sample rate control
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND> --sample-rate 1.5

# Run with both volume and sample rate
cargo run --release -- <PRESS_SOUND> <RELEASE_SOUND> --volume 0.3 --sample-rate 1.2
```

The CLI validates that each sound file argument points to an existing file. Provide absolute paths or paths relative to the current directory.

## Command-Line Options

- `<PRESS_SOUND>` (required) - Path to the audio file to play on key press
- `<RELEASE_SOUND>` (required) - Path to the audio file to play on key release
- `-v, --volume <VOLUME>` (optional) - Volume level for audio playback
  - `0.0` = silent
  - `1.0` = normal volume (default)
  - `2.0` = double volume
  - Can be any positive float value
- `-s, --sample-rate <SAMPLE_RATE>` (optional) - Sample rate multiplier for pitch/speed control
  - `1.0` = normal pitch and speed (default)
  - `2.0` = double speed and high pitch
  - `0.5` = half speed and low pitch
  - Can be any positive float value
- `-h, --help` - Show help information

## Usage Notes

- Supported formats: WAV (PCM) and OGG/Vorbis, decoded via `symphonia`.
- When multiple keys are hit in quick succession the sounds overlap; rodio's `Sink` handles the mixing.
- Volume control affects all audio playback through rodio's sink volume setting.
- Sample rate multiplier changes both pitch and playback speed - higher values create higher-pitched, faster audio.
- The listener targets Wayland keyboard events by default through wl_keyboard protocol.
- Logs report audio metadata, device add/remove events, and keyboard activity.

## Troubleshooting

- **Missing system libraries**: If the build fails with `alsa` or `libudev` not found, install the appropriate development packages or build inside `nix-shell`.
- **No key events**: Ensure your compositor uses libinput and that the process can read `/dev/input/event*`.
- **Permission denied**: Join the `input` group, run via `sudo`, or relax access with a custom udev rule.
- **No audio output**: Check system volume; test the audio files with `aplay path/to/file.wav`.

## Project Structure

```
src/
├── main.rs           # CLI parsing (clap), audio loading, event loop bootstrap
├── audio/            # Audio module (load + playback with volume/sample-rate control)
│   └── mod.rs        # AudioPlayer & AudioSource with volume & sample_rate_multiplier
└── input/            # Input module (Wayland keyboard event handling)
    └── mod.rs        # KeyboardHandler trait & listen() function
```

Core dependencies: `rodio` (audio playback), `symphonia` (audio decoding), `wayland-client` (Wayland protocols), `clap` (CLI parsing), `anyhow` (error handling).

## Known Limitations

- Linux only (`cfg(target_os = "linux")`).
- Requires a physical keyboard managed by libinput; virtual keyboards or remote input setups are untested.
- No automated tests yet—verify behaviour in your target desktop environment.

## License

Released under the MIT License.
