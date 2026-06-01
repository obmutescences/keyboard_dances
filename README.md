# keyboard_dances

`keyboard_dances` is a Linux desktop app for assigning sound effects to keyboard
press and release events. The current version is built with Tauri 2 + Rust and
provides an app window, a system tray icon, persistent config files, and sound
profile switching.

Chinese documentation is available in [README_zh.md](README_zh.md).

Current scope:

- Configure one shared press sound and one shared release sound for all keys.
- Select audio files, save settings, and test playback from the app UI.
- Manage multiple profiles. Switching profiles switches the active press/release sounds.
- Duplicate, rename, and delete profiles from the app UI.
- Listen for keyboard events in the background and play the matching sound.
- Target Linux / NixOS first, with AppImage as the initial packaging format.

## Nix Development Environment

From the repository root, enter the development environment with:

```bash
cd /home/zerone/projects/keyboard_dances
nix develop path:.
```

`path:.` is recommended when `flake.nix` is newly added locally but not yet
tracked by Git. Plain `nix develop` can fail in that state because flakes read
from the Git tree.

If the flake files are already tracked, this also works:

```bash
nix develop
```

Compatibility entry point:

```bash
nix-shell
```

The shell provides the runtime and build dependencies needed by Tauri,
WebKitGTK, GTK, libinput, and ALSA. The frontend is static HTML/CSS/JS, so there
is no `npm install` step and no separate frontend dev server.

For niri / Wayland sessions, the dev shell currently sets:

```text
WEBKIT_DISABLE_COMPOSITING_MODE=1
WEBKIT_DISABLE_DMABUF_RENDERER=1
GDK_BACKEND=x11
```

`GDK_BACKEND=x11` runs GTK / WebKitGTK through Xwayland. This avoids rendering
issues seen on some NixOS + Wayland setups. If the app cannot open a display,
check that your niri session has `xwayland-satellite` installed and enabled. If
your niri session does not use Xwayland, you can temporarily run with
`GDK_BACKEND=wayland cargo tauri dev`.

## Development Run

After entering the Nix shell, run from the repository root:

```bash
cargo tauri dev
```

There is no frontend build step. Tauri dev loads the static files from `ui/`
through the `frontendDist` setting in `src-tauri/tauri.conf.json`.

To run the Rust app directly:

```bash
cargo run --manifest-path src-tauri/Cargo.toml
```

For UI work, prefer `cargo tauri dev` so static asset paths match the final
Tauri app more closely.

## Manual Test Flow

After the app starts, check the following:

1. The main window opens and the UI renders correctly.
2. The system tray icon appears and the tray menu works.
3. Select a Press audio file and a Release audio file in the UI.
4. Save the profile, then use the test buttons to confirm both sounds play.
5. Create, duplicate, rename, delete, and switch profiles.
6. Press and release real keyboard keys, then confirm the background listener
   triggers the matching sounds.

Config files are written to:

```text
~/.config/keyboard-dances/app.toml
~/.config/keyboard-dances/profiles/*.toml
```

Default sample sounds are written to:

```text
~/.local/share/keyboard-dances/sounds/
```

## Linux Input And Wayland Permissions

The app listens to keyboard events through Linux input devices under
`/dev/input/event*`. This is independent of the visible desktop protocol:
Wayland, X11, niri, or Xwayland do not automatically grant global keyboard event
access. The UI can work and the test buttons can play sound while the background
keyboard listener still fails because the user cannot read input devices.

Quick checks:

```bash
groups
ls -l /dev/input/event*
test -r /dev/input/event0
```

Use the actual event device path from your system for the `test -r` command. If
the read check fails, grant access through one of these approaches:

- Add your user to the `input` group.
- Add a narrower udev rule for the keyboard devices you want this app to read.

On NixOS, the user group approach usually looks like this:

```nix
users.users.<your-user>.extraGroups = [ "input" ];
```

Then apply the system config and log out/in so the new group membership is
visible to the desktop session:

```bash
sudo nixos-rebuild switch
```

The `input` group can read raw input events from devices, so treat it as a broad
permission. Prefer a dedicated udev rule if you want to limit access more
tightly.

For niri / Wayland rendering, the dev shell uses `GDK_BACKEND=x11`, so the app
window needs Xwayland support such as `xwayland-satellite`. That display setting
only affects the Tauri/WebKit window; keyboard event listening still depends on
`/dev/input/event*` permissions.

## AppImage Packaging

AppImage is the only packaging target currently prioritized. After manual app
testing passes, enter the Nix shell and build with the NixOS wrapper:

```bash
cd /home/zerone/projects/keyboard_dances
nix develop path:.
scripts/build-appimage-nixos.sh
```

On NixOS, plain `cargo tauri build --bundles appimage` can fail at
`failed to run linuxdeploy` because Tauri uses its cached linuxdeploy AppImage.
The wrapper still uses Tauri to build the binary and prepare the AppDir, then
finishes the AppImage with Nixpkgs `linuxdeploy`.

The AppImage output is written under:

```text
target/release/bundle/appimage/
```

deb / rpm and other package formats can be added later after the AppImage flow is
stable.

## Project Structure

```text
src-tauri/
├── src/
│   ├── main.rs      # Tauri entry point, commands, system tray
│   ├── audio/       # Audio loading and playback
│   ├── input/       # Linux input event listener
│   ├── config.rs    # App config and profile persistence
│   └── runtime.rs   # Background runtime state
├── tauri.conf.json  # Tauri config and AppImage bundle config
└── build.rs         # Generates the default icon at build time

ui/
├── index.html
├── main.js
└── styles.css

nix/
└── dev-shell.nix
```

Core dependencies: Tauri 2, rodio, symphonia, input, directories, rfd, and toml.

## Known Limitations

- Linux only for now.
- All keys currently share one press sound and one release sound.
- Per-key sound configuration is not implemented yet.
- Real keyboard listening depends on `/dev/input/event*` permissions.
- AppImage is the current priority packaging format. Other package formats are
  deferred.

## License

Released under the MIT License.
