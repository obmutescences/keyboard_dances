use anyhow::{Context, Result};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const APP_QUALIFIER: &str = "dev";
const APP_ORGANIZATION: &str = "keyboard-dances";
const APP_NAME: &str = "keyboard-dances";
const DEFAULT_PROFILE_NAME: &str = "default";
const DEFAULT_PRESS_SOUND: &[u8] = include_bytes!("../../ff-0.wav");
const DEFAULT_RELEASE_SOUND: &[u8] = include_bytes!("../../ff-1.wav");

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
        let mut profile = profile.clone();
        profile.name = name;
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
        self.data_dir.join("sounds")
    }

    fn ensure_sample_sound(&self, file_name: &str, bytes: &[u8]) -> Result<()> {
        let path = self.sounds_dir().join(file_name);
        if path.exists() {
            return Ok(());
        }
        fs::write(&path, bytes).with_context(|| format!("Failed to write {}", path.display()))
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
