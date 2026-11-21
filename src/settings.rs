use std::{collections::HashMap, fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme_id: String,
    pub profile: String,
    #[serde(default)]
    pub device_profiles: HashMap<String, String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme_id: "default".into(),
            profile: String::new(),
            device_profiles: HashMap::new(),
        }
    }
}

impl Settings {
    pub fn profile_for(&self, device_id: &str) -> &str {
        self.device_profiles
            .get(device_id)
            .map(String::as_str)
            .unwrap_or(&self.profile)
    }

    pub fn set_profile_for(&mut self, device_id: &str, name: &str) {
        self.device_profiles
            .insert(device_id.to_string(), name.to_string());
        self.profile = name.to_string();
    }
}

pub struct SettingsManager {
    path: PathBuf,
}

impl SettingsManager {
    pub fn new() -> Self {
        let path = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("ds4u")
            .join("settings.json");

        Self { path }
    }

    fn read(&self) -> Option<Settings> {
        let json = fs::read_to_string(&self.path).ok()?;
        serde_json::from_str(&json).ok()
    }

    pub fn load(&self) -> Settings {
        self.read().unwrap_or_default()
    }

    pub fn save(&self, settings: &Settings) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut out = settings.clone();
        if let Some(disk) = self.read() {
            for (id, name) in disk.device_profiles {
                out.device_profiles.entry(id).or_insert(name);
            }
        }

        if let Ok(json) = serde_json::to_string_pretty(&out) {
            let _ = fs::write(&self.path, json);
        }
    }
}
