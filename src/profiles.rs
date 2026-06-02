use std::{
    collections::{HashMap, HashSet},
    fs,
    path::PathBuf,
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    common::*,
    inputs::Button,
    transform::{InputTransform, TriggerDeadband},
};

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
    pub trigger_feedback_position: u8,
    pub haptic_intensity: u8,
    pub gyro_sensetivity: f32,
    pub touchpad_enabled: bool,
    pub button_remapping: HashMap<Button, Button>,
    pub disabled_buttons: HashSet<Button>,
    pub stick_left_deadzone: f32,
    pub stick_right_deadzone: f32,
    pub trigger_left_deadband: TriggerDeadband,
    pub trigger_right_deadband: TriggerDeadband,
}

impl Profile {
    pub fn to_input_transform(&self) -> InputTransform {
        InputTransform {
            left_curve: self.stick_left_curve.clone(),
            right_curve: self.stick_right_curve.clone(),
            left_deadzone: self.stick_left_deadzone,
            right_deadzone: self.stick_right_deadzone,
            trigger_left: self.trigger_left_deadband.clone(),
            trigger_right: self.trigger_right_deadband.clone(),
            button_remap: self.button_remapping.clone(),
            disabled_buttons: self.disabled_buttons.clone(),
        }
    }

    pub fn to_trigger_effect(&self) -> Option<(u8, [u8; 10])> {
        match self.trigger_mode {
            TriggerMode::Off => None,
            TriggerMode::Feedback => {
                let pos = self.trigger_feedback_position.min(9) as usize;
                let str_ = self.haptic_intensity.clamp(1, 8);
                let mut strengths = [0u8; 10];
                for i in pos..10 {
                    strengths[i] = str_;
                }
                let mut active_zones: u16 = 0;
                let mut strength_zones: u32 = 0;
                for i in 0..10 {
                    if strengths[i] > 0 {
                        let sv = ((strengths[i] - 1) & 0x07) as u32;
                        strength_zones |= sv << (3 * i);
                        active_zones |= 1 << i;
                    }
                }
                let params: [u8; 10] = [
                    (active_zones & 0xff) as u8,
                    ((active_zones >> 8) & 0xff) as u8,
                    (strength_zones & 0xff) as u8,
                    ((strength_zones >> 8) & 0xff) as u8,
                    ((strength_zones >> 16) & 0xff) as u8,
                    ((strength_zones >> 24) & 0xff) as u8,
                    0,
                    0,
                    0,
                    0,
                ];
                Some((0x21, params))
            }
            _ => None,
        }
    }
}

pub struct ProfileManager {
    profiles_dir: PathBuf,
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

    pub fn ensure_default_exists(&self) -> Profile {
        if self.profile_exists("Default") {
            self.load_profile("Default").unwrap_or_default()
        } else {
            let profile = Profile::default();
            let _ = self.save_profile(&profile);
            profile
        }
    }

    fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' {
                    c
                } else {
                    '_'
                }
            })
            .collect()
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
                    && let Ok(profile) = serde_json::from_str::<Profile>(&json)
                {
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
            trigger_feedback_position: 0,
            haptic_intensity: 0,
            gyro_sensetivity: 1.0,
            touchpad_enabled: true,
            button_remapping: HashMap::new(),
            disabled_buttons: HashSet::new(),
            stick_left_deadzone: 0.0,
            stick_right_deadzone: 0.0,
            trigger_left_deadband: TriggerDeadband::default(),
            trigger_right_deadband: TriggerDeadband::default(),
        }
    }
}
