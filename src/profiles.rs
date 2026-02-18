use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::common::*;

#[derive(Clone, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub lightbar_r: f32,
    pub lightbar_g: f32,
    pub lightbar_b: f32,
    pub lightbar_brightness: f32,
    pub player_leds: u8,
    pub mic_enabled: bool,
    pub stick_left_curve: SensitivityCurve,
    pub stick_right_curve: SensitivityCurve,
    pub trigger_mode: TriggerMode,
    pub haptic_intensity: u8,
    pub gyro_sensetivity: f32,
    pub touchpad_enabled: bool,
    pub button_remapping: HashMap<Button, Button>
}


pub struct ProfileManager {
    profiles_dir: PathBuf
}

impl ProfileManager {
    pub fn new() -> Self {
        let profiles_dir = Self::get_profiles_dir();

        if !profiles_dir.exists() {
            let _ = fs::create_dir_all(&profiles_dir);
        }

        Self { profiles_dir }
    }

    fn get_profiles_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ds4u")
            .join("profiles")
    }

    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            }).collect()
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<()> {
        let filename = format!("{}.json", Self::sanitize_filename(&profile.name));
        let path = self.profiles_dir.join(filename);

        let json = serde_json::to_string_pretty(profile)?;
        fs::write(path, json)?;

        Ok(())
    }

    pub fn load_profile(&self, name: &str) -> Result<Profile> {
        let filename = format!("{}.json", Self::sanitize_filename(name));
        let path = self.profiles_dir.join(filename);

        if !path.exists() {
            bail!("Profile '{}' not found", name);
        }

        let json = fs::read_to_string(path)?;
        let profile: Profile = serde_json::from_str(&json)?;

        Ok(profile)
    }

    pub fn delete_profile(&self, name: &str) -> Result<()> {
        let filename = format!("{}.json", Self::sanitize_filename(name));
        let path = self.profiles_dir.join(filename);

        if !path.exists() {
            bail!("Profile '{}' not found", name);
        }

        fs::remove_file(path)?;

        Ok(())
    }

    pub fn profile_exists(&self, name: &str) -> bool { 
        let filename = format!("{}.json", Self::sanitize_filename(name));
        self.profiles_dir.join(filename).exists()
    }

    pub fn list_profiles(&self) -> Vec<Profile> {
        let mut profiles = Vec::new();

        if let Ok(entries) = fs::read_dir(&self.profiles_dir) {
            for e in entries.flatten() {
                let path = e.path();

                if path.extension().and_then(|s| s.to_str()) == Some("json") 
                    && let Ok(json) = fs::read_to_string(&path)
                        && let Ok(profile) = serde_json::from_str::<Profile>(&json) {
                            profiles.push(profile);
                }

            }
        }

        profiles
    }
}

impl Clone for ProfileManager {
    fn clone(&self) -> Self {
        ProfileManager::new()
    }
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            name: "Default".to_string(),
            lightbar_r: 0.0,
            lightbar_g: 0.5,
            lightbar_b: 1.0,
            lightbar_brightness: 255.0,
            player_leds: 1,
            mic_enabled: false,
            stick_left_curve: SensitivityCurve::Default,
            stick_right_curve: SensitivityCurve::Default,
            trigger_mode: TriggerMode::Off,
            haptic_intensity: 0,
            gyro_sensetivity: 1.0,
            touchpad_enabled: true,
            button_remapping: HashMap::new()
        }
    }
}
