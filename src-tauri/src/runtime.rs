use crate::audio::AudioPlayer;
use crate::config::{AppConfig, ConfigStore, ProfileConfig, ProfileSummary};
use crate::input::{KeyEvent, KeyboardHandler, listen};
use anyhow::{Context, Result};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Clone)]
pub struct AppRuntime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    store: ConfigStore,
    state: Mutex<RuntimeState>,
}

struct RuntimeState {
    app_config: AppConfig,
    active_profile: ProfileConfig,
    player: Option<AudioPlayer>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeSnapshot {
    pub enabled: bool,
    pub active_profile: ProfileConfig,
    pub profiles: Vec<ProfileSummary>,
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub last_error: Option<String>,
}

impl AppRuntime {
    pub fn new() -> Result<Self> {
        let store = ConfigStore::new()?;
        store.ensure_initialized()?;

        let mut app_config = store.load_app_config().unwrap_or_default();
        let active_profile = match store.load_profile(&app_config.active_profile) {
            Ok(profile) => profile,
            Err(_) => {
                let profile = store.default_profile();
                store.save_profile(&profile)?;
                app_config.active_profile = profile.name.clone();
                store.save_app_config(&app_config)?;
                profile
            }
        };

        let (player, last_error) = load_player(&active_profile);
        let state = RuntimeState {
            app_config,
            active_profile,
            player,
            last_error,
        };

        Ok(Self {
            inner: Arc::new(RuntimeInner {
                store,
                state: Mutex::new(state),
            }),
        })
    }

    pub fn snapshot(&self) -> Result<RuntimeSnapshot> {
        let state = self.lock_state()?;
        self.snapshot_locked(&state)
    }

    pub fn switch_profile(&self, name: &str) -> Result<RuntimeSnapshot> {
        let profile = self.inner.store.load_profile(name)?;
        let (player, last_error) = load_player(&profile);

        let mut state = self.lock_state()?;
        state.active_profile = profile;
        state.player = player;
        state.last_error = last_error;
        state.app_config.active_profile = state.active_profile.name.clone();
        self.inner.store.save_app_config(&state.app_config)?;

        self.snapshot_locked(&state)
    }

    pub fn next_profile(&self) -> Result<RuntimeSnapshot> {
        let current = {
            let state = self.lock_state()?;
            state.app_config.active_profile.clone()
        };
        let profiles = self.inner.store.list_profiles(&current)?;
        if profiles.is_empty() {
            return self.snapshot();
        }

        let current_index = profiles
            .iter()
            .position(|profile| profile.name == current)
            .unwrap_or(0);
        let next_index = (current_index + 1) % profiles.len();
        self.switch_profile(&profiles[next_index].name)
    }

    pub fn save_profile(&self, profile: ProfileConfig) -> Result<RuntimeSnapshot> {
        let profile_name = self.inner.store.save_profile(&profile)?;
        self.switch_profile(&profile_name)
    }

    pub fn save_active_profile(&self, profile: ProfileConfig) -> Result<RuntimeSnapshot> {
        let previous_profile = {
            let state = self.lock_state()?;
            state.app_config.active_profile.clone()
        };
        let existing_profiles = self.inner.store.list_profile_names()?;
        let next_profile = crate::config::normalized_profile_name(&profile.name);
        let previous_profile = crate::config::normalized_profile_name(&previous_profile);

        anyhow::ensure!(!next_profile.is_empty(), "Profile name cannot be empty");
        anyhow::ensure!(
            next_profile == previous_profile
                || !existing_profiles
                    .iter()
                    .any(|profile_name| profile_name == &next_profile),
            "Profile already exists: {next_profile}"
        );

        let saved_profile = self.inner.store.save_profile(&profile)?;
        if saved_profile != previous_profile {
            self.inner.store.delete_profile(&previous_profile)?;
        }
        self.switch_profile(&saved_profile)
    }

    pub fn delete_profile(&self, name: &str) -> Result<RuntimeSnapshot> {
        let active_profile = {
            let state = self.lock_state()?;
            state.app_config.active_profile.clone()
        };
        let active_profile = crate::config::normalized_profile_name(&active_profile);

        let deleted_profile = self.inner.store.delete_profile(name)?;

        if active_profile == deleted_profile {
            let next_profile = self
                .inner
                .store
                .list_profile_names()?
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No profiles remain after deletion"))?;
            self.switch_profile(&next_profile)
        } else {
            self.snapshot()
        }
    }

    pub fn set_enabled(&self, enabled: bool) -> Result<RuntimeSnapshot> {
        let mut state = self.lock_state()?;
        state.app_config.enabled = enabled;
        self.inner.store.save_app_config(&state.app_config)?;
        self.snapshot_locked(&state)
    }

    pub fn test_press(&self) -> Result<()> {
        let state = self.lock_state()?;
        if let Some(player) = &state.player {
            player.play_press();
        }
        Ok(())
    }

    pub fn test_release(&self) -> Result<()> {
        let state = self.lock_state()?;
        if let Some(player) = &state.player {
            player.play_release();
        }
        Ok(())
    }

    pub fn start_listener(&self) -> Result<()> {
        let runtime = self.clone();
        let runtime_for_error = self.clone();

        thread::Builder::new()
            .name("keyboard-dances-input".to_string())
            .spawn(move || {
                let handler = RuntimeKeyboardHandler { runtime };
                if let Err(err) = listen(handler) {
                    runtime_for_error.set_last_error(format!("Input listener stopped: {err:#}"));
                }
            })
            .context("Failed to spawn input listener")?;

        Ok(())
    }

    fn play_event(&self, event: KeyEvent) {
        let Ok(state) = self.lock_state() else {
            return;
        };
        if !state.app_config.enabled {
            return;
        }

        match event {
            KeyEvent::Press => {
                if let Some(player) = &state.player {
                    player.play_press();
                }
            }
            KeyEvent::Release => {
                if let Some(player) = &state.player {
                    player.play_release();
                }
            }
        }
    }

    fn set_last_error(&self, error: String) {
        if let Ok(mut state) = self.lock_state() {
            state.last_error = Some(error);
        }
    }

    fn snapshot_locked(&self, state: &RuntimeState) -> Result<RuntimeSnapshot> {
        Ok(RuntimeSnapshot {
            enabled: state.app_config.enabled,
            active_profile: state.active_profile.clone(),
            profiles: self
                .inner
                .store
                .list_profiles(&state.app_config.active_profile)?,
            config_dir: self.inner.store.config_dir().to_path_buf(),
            data_dir: self.inner.store.data_dir().to_path_buf(),
            profiles_dir: self.inner.store.profiles_dir().to_path_buf(),
            last_error: state.last_error.clone(),
        })
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, RuntimeState>> {
        self.inner
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Application state lock was poisoned"))
    }
}

struct RuntimeKeyboardHandler {
    runtime: AppRuntime,
}

impl KeyboardHandler for RuntimeKeyboardHandler {
    fn on_key_event(&self, event: KeyEvent) {
        self.runtime.play_event(event);
    }
}

fn load_player(profile: &ProfileConfig) -> (Option<AudioPlayer>, Option<String>) {
    if !profile.press_sound.exists() {
        return (
            None,
            Some(format!(
                "Press sound file does not exist: {}",
                profile.press_sound.display()
            )),
        );
    }

    if !profile.release_sound.exists() {
        return (
            None,
            Some(format!(
                "Release sound file does not exist: {}",
                profile.release_sound.display()
            )),
        );
    }

    match AudioPlayer::new(
        &profile.press_sound,
        &profile.release_sound,
        1.0,
        1.0,
    ) {
        Ok(player) => (Some(player), None),
        Err(err) => (None, Some(format!("Failed to load audio: {err:#}"))),
    }
}
