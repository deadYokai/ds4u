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
    transform::{GyroProcessor, InputTransform, TriggerDeadband},
};

#[derive(Clone, Deserialize, Serialize, PartialEq, Debug)]
pub struct TriggerConfig {
    pub mode: TriggerMode,
    pub start: u8,
    pub end: u8,
    pub strength: u8,
    pub frequency: u8,
    pub deadband: TriggerDeadband,
}

impl Default for TriggerConfig {
    fn default() -> Self {
        Self {
            mode: TriggerMode::Off,
            start: 2,
            end: 7,
            strength: 5,
            frequency: 30,
            deadband: TriggerDeadband::default(),
        }
    }
}

impl TriggerConfig {
    pub fn to_effect(&self) -> (u8, [u8; 10]) {
        match self.mode {
            TriggerMode::Off => (0x05, [0; 10]),
            TriggerMode::Feedback => {
                let start = self.start.min(9) as usize;
                let end = self.end.clamp(self.start, 9) as usize;
                let strength = self.strength.clamp(1, 8);
                let mut active_zones: u16 = 0;
                let mut strength_zones: u32 = 0;
                for i in start..=end {
                    let sv = ((strength - 1) & 0x07) as u32;
                    strength_zones |= sv << (3 * i);
                    active_zones |= 1 << i;
                }
                let mut p = [0u8; 10];
                p[0] = (active_zones & 0xff) as u8;
                p[1] = ((active_zones >> 8) & 0xff) as u8;
                p[2] = (strength_zones & 0xff) as u8;
                p[3] = ((strength_zones >> 8) & 0xff) as u8;
                p[4] = ((strength_zones >> 16) & 0xff) as u8;
                p[5] = ((strength_zones >> 24) & 0xff) as u8;
                (0x21, p)
            }
            TriggerMode::Weapon => {
                let start = self.start.clamp(2, 7);
                let end = self.end.clamp(start + 1, 8);
                let strength = self.strength.clamp(1, 8);
                let mut p = [0u8; 10];
                let positions: u16 = (1u16 << start) | (1u16 << end);
                p[0] = (positions & 0xff) as u8;
                p[1] = ((positions >> 8) & 0xff) as u8;
                p[2] = strength - 1;
                (0x25, p)
            }
            TriggerMode::Bow => {
                let start = self.start.min(8);
                let end = self.end.clamp(start + 1, 8);
                let strength = self.strength.clamp(1, 8);
                let snap = self.strength.clamp(1, 8);
                let mut p = [0u8; 10];
                let positions: u16 = (1u16 << start) | (1u16 << end);
                let force: u32 = ((strength - 1) as u32 & 0x07) | (((snap - 1) as u32 & 0x07) << 3);
                p[0] = (positions & 0xff) as u8;
                p[1] = ((positions >> 8) & 0xff) as u8;
                p[2] = (force & 0xff) as u8;
                p[3] = ((force >> 8) & 0xff) as u8;
                (0x22, p)
            }
            TriggerMode::Galloping => {
                let start = self.start.min(8);
                let end = self.end.clamp(start + 1, 9);
                let first_foot = self.strength.saturating_sub(1).min(7);
                let second_foot = first_foot.saturating_add(1).min(7);
                let freq = self.frequency.max(1);
                let mut p = [0u8; 10];
                p[0] = start;
                p[1] = end;
                p[2] = (second_foot << 4) | first_foot;
                p[3] = freq;
                (0x23, p)
            }
            TriggerMode::Vibration => {
                let start = self.start.min(9) as usize;
                let end = self.end.clamp(self.start, 9) as usize;
                let strength = self.strength.clamp(1, 8);
                let freq = self.frequency.max(1);
                let mut active_zones: u16 = 0;
                let mut amp_zones: u32 = 0;
                for i in start..=end {
                    active_zones |= 1 << i;
                    amp_zones |= ((strength - 1) as u32 & 0x07) << (3 * i);
                }
                let mut p = [0u8; 10];
                p[0] = (active_zones & 0xff) as u8;
                p[1] = ((active_zones >> 8) & 0xff) as u8;
                p[2] = (amp_zones & 0xff) as u8;
                p[3] = ((amp_zones >> 8) & 0xff) as u8;
                p[4] = ((amp_zones >> 16) & 0xff) as u8;
                p[5] = ((amp_zones >> 24) & 0xff) as u8;
                p[6] = freq;
                (0x26, p)
            }
            TriggerMode::Machine => {
                let start = self.start.min(8);
                let end = self.end.clamp(start + 1, 9);
                let amp_a = self.strength.clamp(1, 8);
                let amp_b = self.strength.clamp(1, 8);
                let freq = self.frequency.max(1);
                let mut p = [0u8; 10];
                p[0] = start;
                p[1] = end;
                p[2] = ((amp_b - 1) << 4) | (amp_a - 1);
                p[3] = freq;
                p[4] = 0;
                (0x27, p)
            }
        }
    }
}

fn default_touchpad_sensitivity() -> f32 {
    1.0
}

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
    #[serde(default)]
    pub touchpad_mode: TouchpadMode,
    #[serde(default)]
    pub touchpad_tap_to_click: bool,
    #[serde(default)]
    pub touchpad_natural_scrolling: bool,
    #[serde(default = "default_touchpad_sensitivity")]
    pub touchpad_sensitivity: f32,
    pub button_remapping: HashMap<Button, Button>,
    pub disabled_buttons: HashSet<Button>,
    pub stick_left_deadzone: f32,
    pub stick_right_deadzone: f32,
    pub trigger_left_deadband: TriggerDeadband,
    pub trigger_right_deadband: TriggerDeadband,
    #[serde(default)]
    pub stick_left_outer_deadzone: f32,
    #[serde(default)]
    pub stick_right_outer_deadzone: f32,
    #[serde(default)]
    pub stick_left_invert_x: bool,
    #[serde(default)]
    pub stick_left_invert_y: bool,
    #[serde(default)]
    pub stick_right_invert_x: bool,
    #[serde(default)]
    pub stick_right_invert_y: bool,
    #[serde(default)]
    pub stick_swap: bool,

    #[serde(default)]
    pub trigger_left_config: TriggerConfig,
    #[serde(default)]
    pub trigger_right_config: TriggerConfig,

    #[serde(default)]
    pub gyro: GyroProcessor,

    #[serde(default)]
    pub haptic_pattern: HapticPattern,
    #[serde(default)]
    pub haptic_strength: u8, // 0-7
    #[serde(default)]
    pub haptic_speed: f32, // Hz

    #[serde(default)]
    pub touchpad_show_overlay: bool,
}

impl Profile {
    pub fn to_input_transform(&self) -> InputTransform {
        InputTransform {
            left_curve: self.stick_left_curve.clone(),
            right_curve: self.stick_right_curve.clone(),
            left_deadzone: self.stick_left_deadzone,
            right_deadzone: self.stick_right_deadzone,
            left_outer_deadzone: self.stick_left_outer_deadzone,
            right_outer_deadzone: self.stick_right_outer_deadzone,
            left_invert_x: self.stick_left_invert_x,
            left_invert_y: self.stick_left_invert_y,
            right_invert_x: self.stick_right_invert_x,
            right_invert_y: self.stick_right_invert_y,
            stick_swap: self.stick_swap,
            trigger_left: self.trigger_left_config.deadband.clone(),
            trigger_right: self.trigger_right_config.deadband.clone(),
            button_remap: self.button_remapping.clone(),
            disabled_buttons: self.disabled_buttons.clone(),
            touchpad_enabled: self.touchpad_enabled,
            touchpad_mode: self.touchpad_mode,
        }
    }

    pub fn to_gyro_processor(&self) -> GyroProcessor {
        GyroProcessor {
            enabled: self.gyro.enabled,
            smoothing: self.gyro.smoothing,
            sensitivity: self.gyro.sensitivity * self.gyro_sensetivity,
            ..Default::default()
        }
    }

    pub fn to_trigger_effect(&self) -> Option<(u8, [u8; 10])> {
        match self.trigger_mode {
            TriggerMode::Off => None,
            _ => Some(self.trigger_left_config.to_effect()),
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

    pub fn validate_profile_name(name: &str) -> Result<()> {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            bail!("Profile name cannot be empty");
        }

        if trimmed.len() > 64 {
            bail!("Profile name must be less than 64 characters");
        }

        if Self::sanitize_filename(trimmed) != trimmed {
            bail!(
                "Profile name '{}' contains invalid characters. Allowed: letters, digits, '-', '_' (no spaces, slashes, or punctuation)",
                name
            );
        }
        Ok(())
    }

    pub fn save_profile(&self, profile: &Profile) -> Result<()> {
        Self::validate_profile_name(&profile.name)?;
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
        if name == "Default" {
            bail!("Cannot delete Default profile");
        }
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
        profiles.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
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
            touchpad_mode: TouchpadMode::Mouse,
            touchpad_tap_to_click: true,
            touchpad_natural_scrolling: false,
            touchpad_sensitivity: 1.0,
            button_remapping: HashMap::new(),
            disabled_buttons: HashSet::new(),
            stick_left_deadzone: 0.0,
            stick_right_deadzone: 0.0,
            trigger_left_deadband: TriggerDeadband::default(),
            trigger_right_deadband: TriggerDeadband::default(),

            stick_left_outer_deadzone: 1.0,
            stick_right_outer_deadzone: 1.0,
            stick_left_invert_x: false,
            stick_left_invert_y: false,
            stick_right_invert_x: false,
            stick_right_invert_y: false,
            stick_swap: false,

            trigger_left_config: TriggerConfig::default(),
            trigger_right_config: TriggerConfig::default(),

            gyro: GyroProcessor::default(),

            haptic_pattern: HapticPattern::None,
            haptic_strength: 0,
            haptic_speed: 1.0,

            touchpad_show_overlay: true,
        }
    }
}
