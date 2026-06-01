mod audio;
mod config;
mod input;
mod runtime;

use crate::config::ProfileConfig;
use crate::runtime::{AppRuntime, RuntimeSnapshot};
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager};

type CommandResult<T> = Result<T, String>;

#[tauri::command]
fn get_state(runtime: tauri::State<'_, AppRuntime>) -> CommandResult<RuntimeSnapshot> {
    runtime.snapshot().map_err(to_command_error)
}

#[tauri::command]
fn switch_profile(
    runtime: tauri::State<'_, AppRuntime>,
    name: String,
) -> CommandResult<RuntimeSnapshot> {
    runtime.switch_profile(&name).map_err(to_command_error)
}

#[tauri::command]
fn next_profile(runtime: tauri::State<'_, AppRuntime>) -> CommandResult<RuntimeSnapshot> {
    runtime.next_profile().map_err(to_command_error)
}

#[tauri::command]
fn save_profile(
    runtime: tauri::State<'_, AppRuntime>,
    profile: ProfileConfig,
) -> CommandResult<RuntimeSnapshot> {
    runtime.save_profile(profile).map_err(to_command_error)
}

#[tauri::command]
fn save_active_profile(
    runtime: tauri::State<'_, AppRuntime>,
    profile: ProfileConfig,
) -> CommandResult<RuntimeSnapshot> {
    runtime.save_active_profile(profile).map_err(to_command_error)
}

#[tauri::command]
fn delete_profile(
    runtime: tauri::State<'_, AppRuntime>,
    name: String,
) -> CommandResult<RuntimeSnapshot> {
    runtime.delete_profile(&name).map_err(to_command_error)
}

#[tauri::command]
fn set_enabled(
    runtime: tauri::State<'_, AppRuntime>,
    enabled: bool,
) -> CommandResult<RuntimeSnapshot> {
    runtime.set_enabled(enabled).map_err(to_command_error)
}

#[tauri::command]
fn test_press(runtime: tauri::State<'_, AppRuntime>) -> CommandResult<()> {
    runtime.test_press().map_err(to_command_error)
}

#[tauri::command]
fn test_release(runtime: tauri::State<'_, AppRuntime>) -> CommandResult<()> {
    runtime.test_release().map_err(to_command_error)
}

#[tauri::command]
fn pick_sound_file() -> Option<String> {
    rfd::FileDialog::new()
        .add_filter("Audio", &["wav", "ogg"])
        .pick_file()
        .map(|path| path.to_string_lossy().into_owned())
}

fn main() -> anyhow::Result<()> {
    let runtime = AppRuntime::new()?;
    let runtime_for_setup = runtime.clone();

    tauri::Builder::default()
        .manage(runtime)
        .setup(move |app| {
            runtime_for_setup.start_listener()?;
            setup_tray(app)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            switch_profile,
            next_profile,
            save_profile,
            save_active_profile,
            delete_profile,
            set_enabled,
            test_press,
            test_release,
            pick_sound_file
        ])
        .run(tauri::generate_context!())?;

    Ok(())
}

fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Keyboard Dances", true, None::<&str>)?;
    let next_profile = MenuItem::with_id(app, "next_profile", "Next Profile", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle_enabled", "Pause or Resume", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &next_profile, &toggle, &quit])?;

    TrayIconBuilder::new()
        .icon(make_tray_icon())
        .tooltip("Keyboard Dances")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "next_profile" => {
                let runtime = app.state::<AppRuntime>();
                if runtime.next_profile().is_ok() {
                    emit_runtime_state(app);
                }
            }
            "toggle_enabled" => {
                let runtime = app.state::<AppRuntime>();
                if let Ok(snapshot) = runtime.snapshot() {
                    let _ = runtime.set_enabled(!snapshot.enabled);
                    emit_runtime_state(app);
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(&tray.app_handle());
            }
        })
        .build(app)?;

    Ok(())
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn emit_runtime_state(app: &tauri::AppHandle) {
    let runtime = app.state::<AppRuntime>();
    if let Ok(snapshot) = runtime.snapshot() {
        let _ = app.emit("runtime-state-changed", snapshot);
    }
}

fn make_tray_icon() -> Image<'static> {
    let size = 32;
    let mut rgba = Vec::with_capacity(size * size * 4);

    for y in 0..size {
        for x in 0..size {
            let border = x == 5 || x == 26 || y == 8 || y == 23;
            let key_body = (6..=25).contains(&x) && (9..=22).contains(&y);
            let dot = ((x as i32 - 12).pow(2) + (y as i32 - 15).pow(2)) < 7
                || ((x as i32 - 20).pow(2) + (y as i32 - 15).pow(2)) < 7;

            let pixel = if border {
                [29, 45, 54, 255]
            } else if dot {
                [255, 193, 84, 255]
            } else if key_body {
                [80, 173, 132, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&pixel);
        }
    }

    Image::new_owned(rgba, size as u32, size as u32)
}

fn to_command_error(error: anyhow::Error) -> String {
    format!("{error:#}")
}
