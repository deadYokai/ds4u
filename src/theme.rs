use std::{fs, path::PathBuf};

use egui::Color32;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ThemeColors {
    pub window_bg: [u8; 3],
    pub panel_bg: [u8; 3],
    pub extreme_bg: [u8; 3],
    pub accent: [u8; 3],
    pub widget_hovered: [u8; 3],
    pub widget_inactive: [u8; 3],
    pub text: [u8; 3],
    pub text_dim: [u8; 3],
    pub success: [u8; 3],
    pub error: [u8; 3],
}

impl ThemeColors {
    #[inline] pub fn window_bg(&self) -> Color32 { c(self.window_bg) }
    #[inline] pub fn panel_bg(&self) -> Color32 { c(self.panel_bg) }
    #[inline] pub fn extreme_bg(&self) -> Color32 { c(self.extreme_bg) }
    #[inline] pub fn accent(&self) -> Color32 { c(self.accent) }
    #[inline] pub fn widget_hovered(&self) -> Color32 { c(self.widget_hovered) }
    #[inline] pub fn widget_inactive(&self) -> Color32 { c(self.widget_inactive) }
    #[inline] pub fn text(&self) -> Color32 { c(self.text) }
    #[inline] pub fn text_dim(&self) -> Color32 { c(self.text_dim) }
    #[inline] pub fn success(&self) -> Color32 { c(self.success) }
    #[inline] pub fn error(&self) -> Color32 { c(self.error) }
}

#[inline] fn c([r, g, b]: [u8; 3]) -> Color32 { Color32::from_rgb(r, g, b) }

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub dark_mode: bool,
    pub colors: ThemeColors
}

pub fn builtin_themes() -> Vec<Theme> {
    vec![
        default(),
        deep_dark(),
        tokyo_night()
    ]
}

pub fn default_theme() -> Theme { default() }

pub fn theme_by_id(id: &str) -> Theme {
    builtin_themes()
        .into_iter().find(|t| t.id == id)
        .unwrap_or_else(default_theme)
}

pub fn default() -> Theme {
    Theme {
        id: "default".into(),
        dark_mode: true,
        name: "Default".into(),
        colors: ThemeColors {
            window_bg:       [12,  18,  28 ],
            panel_bg:        [16,  24,  36 ],
            extreme_bg:      [8,   12,  20 ],
            accent:          [0,   122, 250],
            widget_hovered:  [40,  60,  90 ],
            widget_inactive: [30,  45,  70 ],
            text:            [240, 240, 250],
            text_dim:        [140, 150, 170],
            success:         [0,   200, 100],
            error:           [255, 80,  80 ],
        },
    }
}

pub fn deep_dark() -> Theme {
    Theme {
        id: "deep_dark".into(),
        dark_mode: true,
        name: "Deep Dark".into(),
        colors: ThemeColors {
            window_bg:       [0,   0,   0  ],
            panel_bg:        [8,   8,   8  ],
            extreme_bg:      [0,   0,   0  ],
            accent:          [0,   122, 255],
            widget_hovered:  [30,  30,  40 ],
            widget_inactive: [20,  20,  28 ],
            text:            [245, 245, 255],
            text_dim:        [120, 120, 140],
            success:         [30,  215, 96 ],
            error:           [255, 69,  58 ],
        },
    }
}

pub fn tokyo_night() -> Theme {
    Theme {
        id: "tokyo_night".into(),
        dark_mode: true,
        name: "Tokyo Night".into(),
        colors: ThemeColors {
            window_bg:       [26,  27,  38 ],  // bg
            panel_bg:        [22,  22,  30 ],  // bg_dark
            extreme_bg:      [16,  16,  24 ],  // bg_darker
            accent:          [122, 162, 247],  // blue
            widget_hovered:  [41,  46,  66 ],  // bg_highlight
            widget_inactive: [32,  36,  54 ],  // bg_visual
            text:            [192, 202, 245],  // fg
            text_dim:        [86,  95,  137],  // comment
            success:         [158, 206, 106],  // green
            error:           [247, 118, 142],  // red
        },
    }
}

pub struct ThemeManager {
    dir: PathBuf
}

impl ThemeManager {
    pub fn new() -> Self {
        let dir = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."))
            .join("ds4u").join("themes");

        let _ = fs::create_dir_all(&dir);
        Self { dir }
    }

    fn theme_path(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", id))
    }

    pub fn load_by_id(&self, id: &str) -> Theme {
        if let Ok(json) = fs::read_to_string(self.theme_path(id))
            && let Ok(t) = serde_json::from_str::<Theme>(&json)
        {
            return t;
        }

        builtin_themes().into_iter().find(|t| t.id == id)
            .unwrap_or_else(default_theme)
    }

    pub fn save_theme(&self, theme: &Theme) {
        let _ = fs::create_dir_all(&self.dir);
        if let Ok(json) = serde_json::to_string_pretty(theme) {
            let _ = fs::write(self.theme_path(&theme.id), json);
        }
    }

    pub fn list_all(&self) -> Vec<Theme> {
        let mut themes = builtin_themes();

        let Ok(entries) = fs::read_dir(&self.dir) else { return themes };

        for e in entries.flatten() {
            let path = e.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let Ok(json) = fs::read_to_string(&path) else { continue };
            let Ok(t) = serde_json::from_str::<Theme>(&json) else { continue };

            if let Some(existing) = themes.iter_mut().find(|e| e.id == t.id) {
                *existing = t;
            } else {
                themes.push(t);
            }
        }

        themes
    }
}

