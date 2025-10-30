use clap::Parser;
use std::path::PathBuf;

mod audio;
mod input;

use crate::audio::AudioPlayer;
use crate::input::{listen, KeyboardHandler};

#[derive(Parser, Debug)]
#[command(name = "keyboard_dances")]
#[command(about = "Play sounds when pressing keys on Wayland")]
struct Args {
    /// Path to the sound file for key press
    #[arg(value_name = "PRESS_SOUND")]
    press_sound: PathBuf,

    /// Path to the sound file for key release
    #[arg(value_name = "RELEASE_SOUND")]
    release_sound: PathBuf,
}

struct KeyHandler {
    player: AudioPlayer,
}

impl KeyHandler {
    fn new(player: AudioPlayer) -> Self {
        Self { player }
    }
}

impl KeyboardHandler for KeyHandler {
    fn on_key_event(&self, event: crate::input::KeyEvent) {
        match event {
            crate::input::KeyEvent::Press => {
                // println!("[Audio] 🔊 Playing PRESS sound");
                self.player.play_press();
            }
            crate::input::KeyEvent::Release => {
                // println!("[Audio] 🔊 Playing RELEASE sound");
                self.player.play_release();
            }
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    // Validate input files
    if !args.press_sound.exists() {
        anyhow::bail!("Press sound file not found: {}", args.press_sound.display());
    }

    if !args.release_sound.exists() {
        anyhow::bail!(
            "Release sound file not found: {}",
            args.release_sound.display()
        );
    }

    println!("Loading audio files...");
    println!("Press sound: {}", args.press_sound.display());
    println!("Release sound: {}", args.release_sound.display());

    let player = AudioPlayer::new(&args.press_sound, &args.release_sound)?;

    // Test audio playback
    println!("Testing audio playback...");
    player.play_press();
    std::thread::sleep(std::time::Duration::from_millis(200));
    player.play_release();
    std::thread::sleep(std::time::Duration::from_millis(200));
    println!("Audio test completed.");

    let handler = KeyHandler::new(player);

    println!("Starting keyboard listener on Wayland...");
    println!("Press any key to hear sounds. Ctrl+C to exit.");

    listen(handler)?;

    Ok(())
}
