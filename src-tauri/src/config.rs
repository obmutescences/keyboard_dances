use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

const APP_QUALIFIER: &str = "dev";
const APP_ORGANIZATION: &str = "keyboard-dances";
const APP_NAME: &str = "keyboard-dances";
const DEFAULT_PROFILE_NAME: &str = "default";
const DEFAULT_PRESS_SOUND: &[u8] = include_bytes!("../../ff-0.wav");
const DEFAULT_RELEASE_SOUND: &[u8] = include_bytes!("../../ff-1.wav");
static SOUND_IMPORT_SEQUENCE: AtomicU64 = AtomicU64::new(0);

struct StagedSound {
    destination: PathBuf,
    staged_path: Option<PathBuf>,
}

impl StagedSound {
    fn commit(mut self) -> Result<PathBuf> {
        if let Some(staged_path) = &self.staged_path {
            fs::rename(staged_path, &self.destination).with_context(|| {
                format!(
                    "Failed to move imported sound to {}",
                    self.destination.display()
                )
            })?;
        }
        self.staged_path = None;
        Ok(self.destination.clone())
    }
}

impl Drop for StagedSound {
    fn drop(&mut self) {
        if let Some(staged_path) = &self.staged_path {
            let _ = fs::remove_file(staged_path);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub active_profile: String,
    pub enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            active_profile: DEFAULT_PROFILE_NAME.to_string(),
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProfileConfig {
    pub name: String,
    pub press_sound: PathBuf,
    pub release_sound: PathBuf,
}

impl Default for ProfileConfig {
    fn default() -> Self {
        Self {
            name: DEFAULT_PROFILE_NAME.to_string(),
            press_sound: PathBuf::new(),
            release_sound: PathBuf::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ProfileSummary {
    pub name: String,
    pub path: PathBuf,
    pub active: bool,
}

#[derive(Debug, Clone)]
pub struct ConfigStore {
    config_dir: PathBuf,
    data_dir: PathBuf,
    profiles_dir: PathBuf,
    app_config_path: PathBuf,
}

impl ConfigStore {
    pub fn new() -> Result<Self> {
        let dirs = ProjectDirs::from(APP_QUALIFIER, APP_ORGANIZATION, APP_NAME)
            .context("Unable to locate a user config directory")?;
        let config_dir = dirs.config_dir().to_path_buf();
        let data_dir = dirs.data_dir().to_path_buf();
        let profiles_dir = config_dir.join("profiles");
        let app_config_path = config_dir.join("app.toml");

        Ok(Self {
            config_dir,
            data_dir,
            profiles_dir,
            app_config_path,
        })
    }

    pub fn ensure_initialized(&self) -> Result<()> {
        fs::create_dir_all(&self.config_dir)
            .with_context(|| format!("Failed to create {}", self.config_dir.display()))?;
        fs::create_dir_all(&self.profiles_dir)
            .with_context(|| format!("Failed to create {}", self.profiles_dir.display()))?;
        fs::create_dir_all(self.sounds_dir())
            .with_context(|| format!("Failed to create {}", self.sounds_dir().display()))?;

        self.ensure_sample_sound("default-press.wav", DEFAULT_PRESS_SOUND)?;
        self.ensure_sample_sound("default-release.wav", DEFAULT_RELEASE_SOUND)?;

        if !self.app_config_path.exists() {
            self.save_app_config(&AppConfig::default())?;
        }

        if self.list_profile_names()?.is_empty() {
            self.save_profile(&self.default_profile())?;
        }

        Ok(())
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn profiles_dir(&self) -> &Path {
        &self.profiles_dir
    }

    pub fn load_app_config(&self) -> Result<AppConfig> {
        let text = fs::read_to_string(&self.app_config_path)
            .with_context(|| format!("Failed to read {}", self.app_config_path.display()))?;
        toml::from_str(&text)
            .with_context(|| format!("Failed to parse {}", self.app_config_path.display()))
    }

    pub fn save_app_config(&self, config: &AppConfig) -> Result<()> {
        let text = toml::to_string_pretty(config).context("Failed to serialize app config")?;
        fs::write(&self.app_config_path, text)
            .with_context(|| format!("Failed to write {}", self.app_config_path.display()))
    }

    pub fn load_profile(&self, name: &str) -> Result<ProfileConfig> {
        let path = self.profile_path(name);
        let text = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        let mut profile: ProfileConfig =
            toml::from_str(&text).with_context(|| format!("Failed to parse {}", path.display()))?;
        if profile.name.trim().is_empty() {
            profile.name = name.to_string();
        }
        Ok(profile)
    }

    pub fn save_profile(&self, profile: &ProfileConfig) -> Result<String> {
        let name = normalized_profile_name(&profile.name);
        anyhow::ensure!(!name.is_empty(), "Profile name cannot be empty");

        fs::create_dir_all(self.sounds_dir())
            .with_context(|| format!("Failed to create {}", self.sounds_dir().display()))?;

        // Stage both sources before replacing either managed file so swapping
        // press and release sounds cannot overwrite the second source.
        let press_sound = self.stage_sound(&name, "press", &profile.press_sound)?;
        let release_sound = self.stage_sound(&name, "release", &profile.release_sound)?;

        let mut profile = profile.clone();
        profile.name = name;
        profile.press_sound = press_sound.commit()?;
        profile.release_sound = release_sound.commit()?;
        let path = self.profile_path(&profile.name);
        let text = toml::to_string_pretty(&profile).context("Failed to serialize profile")?;
        fs::write(&path, text).with_context(|| format!("Failed to write {}", path.display()))?;
        Ok(profile.name)
    }

    pub fn delete_profile(&self, name: &str) -> Result<String> {
        let name = normalized_profile_name(name);
        anyhow::ensure!(!name.is_empty(), "Profile name cannot be empty");

        let names = self.list_profile_names()?;
        anyhow::ensure!(
            names.iter().any(|profile_name| profile_name == &name),
            "Profile does not exist: {name}"
        );
        anyhow::ensure!(names.len() > 1, "Cannot delete the last profile");

        let path = self.profile_path(&name);
        fs::remove_file(&path).with_context(|| format!("Failed to delete {}", path.display()))?;
        Ok(name)
    }

    pub fn list_profiles(&self, active_profile: &str) -> Result<Vec<ProfileSummary>> {
        let mut profiles = Vec::new();
        for name in self.list_profile_names()? {
            profiles.push(ProfileSummary {
                path: self.profile_path(&name),
                active: name == active_profile,
                name,
            });
        }
        Ok(profiles)
    }

    pub fn profile_path(&self, name: &str) -> PathBuf {
        self.profiles_dir
            .join(format!("{}.toml", normalized_profile_name(name)))
    }

    pub fn default_profile(&self) -> ProfileConfig {
        ProfileConfig {
            name: DEFAULT_PROFILE_NAME.to_string(),
            press_sound: self.sounds_dir().join("default-press.wav"),
            release_sound: self.sounds_dir().join("default-release.wav"),
        }
    }

    pub fn list_profile_names(&self) -> Result<Vec<String>> {
        let mut names = Vec::new();
        if !self.profiles_dir.exists() {
            return Ok(names);
        }

        for entry in fs::read_dir(&self.profiles_dir)
            .with_context(|| format!("Failed to read {}", self.profiles_dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
                continue;
            }
            if let Some(name) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(name.to_string());
            }
        }

        names.sort();
        Ok(names)
    }

    fn sounds_dir(&self) -> PathBuf {
        self.config_dir.join("sounds")
    }

    fn stage_sound(&self, profile_name: &str, role: &str, source: &Path) -> Result<StagedSound> {
        anyhow::ensure!(
            source.is_file(),
            "{} sound file does not exist: {}",
            capitalize(role),
            source.display()
        );

        let extension = source
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .with_context(|| format!("Audio file has no valid extension: {}", source.display()))?;
        anyhow::ensure!(
            matches!(extension.as_str(), "wav" | "ogg"),
            "Unsupported audio format .{extension}; choose a WAV or OGG file"
        );

        let destination = self
            .sounds_dir()
            .join(format!("{profile_name}-{role}.{extension}"));
        if paths_refer_to_same_file(source, &destination)? {
            return Ok(StagedSound {
                destination,
                staged_path: None,
            });
        }

        let sequence = SOUND_IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let file_name = destination
            .file_name()
            .and_then(|name| name.to_str())
            .context("Managed sound file name is not valid UTF-8")?;
        let staged_path = self.sounds_dir().join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        fs::copy(source, &staged_path)
            .with_context(|| format!("Failed to copy {} sound from {}", role, source.display()))?;

        Ok(StagedSound {
            destination,
            staged_path: Some(staged_path),
        })
    }

    fn ensure_sample_sound(&self, file_name: &str, bytes: &[u8]) -> Result<()> {
        let path = self.sounds_dir().join(file_name);
        if path.exists() {
            return Ok(());
        }
        fs::write(&path, bytes).with_context(|| format!("Failed to write {}", path.display()))
    }
}

fn paths_refer_to_same_file(source: &Path, destination: &Path) -> Result<bool> {
    if source == destination {
        return Ok(true);
    }
    if !destination.exists() {
        return Ok(false);
    }

    let source = fs::canonicalize(source)
        .with_context(|| format!("Failed to resolve {}", source.display()))?;
    let destination = fs::canonicalize(destination)
        .with_context(|| format!("Failed to resolve {}", destination.display()))?;
    Ok(source == destination)
}

fn capitalize(value: &str) -> String {
    let mut characters = value.chars();
    match characters.next() {
        Some(first) => first.to_uppercase().collect::<String>() + characters.as_str(),
        None => String::new(),
    }
}

pub(crate) fn normalized_profile_name(name: &str) -> String {
    name.trim()
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestStore {
        root: PathBuf,
        store: ConfigStore,
    }

    impl TestStore {
        fn new() -> Self {
            let sequence = SOUND_IMPORT_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "keyboard-dances-config-test-{}-{}",
                std::process::id(),
                sequence
            ));
            let config_dir = root.join("config");
            let data_dir = root.join("data");
            let profiles_dir = config_dir.join("profiles");
            fs::create_dir_all(&profiles_dir).expect("test profile directory should be created");

            let store = ConfigStore {
                app_config_path: config_dir.join("app.toml"),
                config_dir,
                data_dir,
                profiles_dir,
            };
            Self { root, store }
        }

        fn source(&self, file_name: &str, bytes: &[u8]) -> PathBuf {
            let directory = self.root.join("imports");
            fs::create_dir_all(&directory).expect("test import directory should be created");
            let path = directory.join(file_name);
            fs::write(&path, bytes).expect("test sound should be written");
            path
        }
    }

    impl Drop for TestStore {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn save_profile_imports_audio_into_config_sounds_directory() {
        let fixture = TestStore::new();
        let press_source = fixture.source("mechanical.WAV", b"press-audio");
        let release_source = fixture.source("mechanical.ogg", b"release-audio");

        let saved_name = fixture
            .store
            .save_profile(&ProfileConfig {
                name: "Desk Mode".to_string(),
                press_sound: press_source,
                release_sound: release_source,
            })
            .expect("profile should be saved");

        assert_eq!(saved_name, "Desk_Mode");
        let profile = fixture
            .store
            .load_profile(&saved_name)
            .expect("saved profile should load");
        assert_eq!(
            profile.press_sound,
            fixture.store.config_dir.join("sounds/Desk_Mode-press.wav")
        );
        assert_eq!(
            profile.release_sound,
            fixture
                .store
                .config_dir
                .join("sounds/Desk_Mode-release.ogg")
        );
        assert_eq!(
            fs::read(profile.press_sound).expect("managed press sound should be readable"),
            b"press-audio"
        );
        assert_eq!(
            fs::read(profile.release_sound).expect("managed release sound should be readable"),
            b"release-audio"
        );
    }

    #[test]
    fn save_profile_can_resave_and_swap_managed_sounds() {
        let fixture = TestStore::new();
        let profile_name = fixture
            .store
            .save_profile(&ProfileConfig {
                name: "swap".to_string(),
                press_sound: fixture.source("press.wav", b"press-audio"),
                release_sound: fixture.source("release.wav", b"release-audio"),
            })
            .expect("initial profile should be saved");
        let profile = fixture
            .store
            .load_profile(&profile_name)
            .expect("initial profile should load");

        fixture
            .store
            .save_profile(&profile)
            .expect("managed paths should be reusable");
        fixture
            .store
            .save_profile(&ProfileConfig {
                name: profile.name,
                press_sound: profile.release_sound.clone(),
                release_sound: profile.press_sound.clone(),
            })
            .expect("managed sounds should be swappable");

        assert_eq!(
            fs::read(profile.press_sound).expect("swapped press sound should be readable"),
            b"release-audio"
        );
        assert_eq!(
            fs::read(profile.release_sound).expect("swapped release sound should be readable"),
            b"press-audio"
        );
    }
}
