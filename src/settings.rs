use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub theme_id: String,
    pub profile: String
}

impl Default for Settings {
    fn default() -> Self {
        Self { theme_id: "default".into(), profile: String::new() }
    }
}

pub struct SettingsManager {
    path: PathBuf
}

impl SettingsManager {
    pub fn new() -> Self {
        let path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
            .join("ds4u").join("settings.json");

        Self { path }
    }

    pub fn load(&self) -> Settings {
        let Ok(json) = fs::read_to_string(&self.path) else {
            return Settings::default();
        };

        serde_json::from_str(&json).unwrap_or_default()
    }

    pub fn save(&self, settings: &Settings) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        if let Ok(json) = serde_json::to_string_pretty(settings) {
            let _ = fs::write(&self.path, json);
        }
    }
}
