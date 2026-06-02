use anyhow::Result;

// Define the event type
#[derive(Debug, Clone)]
pub enum KeyEvent {
    Press,
    Release,
}

// Handler trait for keyboard events
pub trait KeyboardHandler: 'static + Send {
    fn on_key_event(&self, event: KeyEvent);
}

#[cfg(target_os = "linux")]
pub fn listen<H: KeyboardHandler>(handler: H) -> Result<()> {
    use input::event::device::DeviceEvent;
    use input::event::keyboard::{KeyState, KeyboardEvent, KeyboardEventTrait};
    use input::event::{Event, EventTrait};
    use input::{Libinput, LibinputInterface};
    use libc::O_ACCMODE;
    use std::fs::{File, OpenOptions};
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::io::OwnedFd;
    use std::path::Path;
    use std::{thread, time::Duration};

    struct WaylandInterface;

    impl LibinputInterface for WaylandInterface {
        fn open_restricted(&mut self, path: &Path, flags: i32) -> Result<OwnedFd, i32> {
            let access_mode = flags & O_ACCMODE;
            let mut options = OpenOptions::new();

            // Preserve the caller-specified flags but let OpenOptions control the access mode.
            options.custom_flags((flags & !O_ACCMODE) | libc::O_CLOEXEC);

            if access_mode != libc::O_WRONLY {
                options.read(true);
            }
            if access_mode != libc::O_RDONLY {
                options.write(true);
            }

            options
                .open(path)
                .map(|file| file.into())
                .map_err(|err| err.raw_os_error().unwrap_or(libc::EIO))
        }

        fn close_restricted(&mut self, fd: OwnedFd) {
            drop(File::from(fd));
        }
    }

    println!("[Input] Initializing libinput (Wayland) keyboard listener...");
    let mut libinput = Libinput::new_with_udev(WaylandInterface);
    libinput
        .udev_assign_seat("seat0")
        .map_err(|_| anyhow::anyhow!("Failed to assign seat0 via libinput"))?;

    println!("[Input] Waiting for keyboard events via libinput...");

    loop {
        libinput
            .dispatch()
            .map_err(|e| anyhow::anyhow!("libinput dispatch error: {}", e))?;

        for event in &mut libinput {
            match event {
                Event::Keyboard(KeyboardEvent::Key(key)) => {
                    // let keycode = key.key();
                    // let device_name = key.device().name().to_owned();
                    match key.key_state() {
                        KeyState::Pressed => {
                            // println!(
                            //     "[Input] ✓ Key PRESS detected from {} (keycode: {})",
                            //     device_name, keycode
                            // );
                            handler.on_key_event(KeyEvent::Press);
                        }
                        KeyState::Released => {
                            // println!(
                            //     "[Input] ✓ Key RELEASE detected from {} (keycode: {})",
                            //     device_name, keycode
                            // );
                            handler.on_key_event(KeyEvent::Release);
                        }
                    }
                }
                Event::Device(device_event) => {
                    let device_name = device_event.device().name().to_owned();
                    match device_event {
                        DeviceEvent::Added(_) => {
                            println!("[Input] Device added: {}", device_name);
                        }
                        DeviceEvent::Removed(_) => {
                            println!("[Input] Device removed: {}", device_name);
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        // Avoid busy loop if no events are available.
        thread::sleep(Duration::from_millis(5));
    }
}

// Non-Linux placeholder
#[cfg(not(target_os = "linux"))]
pub fn listen<H: KeyboardHandler>(_handler: H) -> Result<()> {
    use anyhow::bail;
    println!("[Input] Keyboard listening only supported on Linux");
    bail!("Keyboard listening only supported on Linux")
}
