use egui::{Color32, Ui};

use crate::ui::widgets::{
    ds_label, ds_pill_button, ds_row, ds_section, ds_slider, ds_swatch, ds_value_pct, ds_value_text,
};
use crate::{app::DS4UApp, common::LightbarEffect};

impl DS4UApp {
    pub(crate) fn render_lightbar_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ds_section(ui, &c, "Color");

                ds_row(ui, |ui| {
                    ds_label(ui, "RGB Picker");
                    let mut color = [self.lightbar.r, self.lightbar.g, self.lightbar.b];
                    if ui.color_edit_button_rgb(&mut color).changed() {
                        self.lightbar.r = color[0];
                        self.lightbar.g = color[1];
                        self.lightbar.b = color[2];
                        self.apply_lightbar();
                        self.sync_profile();
                    }
                });

                const PRESETS: &[(&str, f32, f32, f32)] = &[
                    ("Aesthetic", 0.0, 0.5, 1.0),
                    ("DevColor", 1.0, 0.63, 0.0),
                    ("Blue", 0.0, 0.0, 1.0),
                    ("Red", 1.0, 0.0, 0.0),
                    ("Green", 0.0, 1.0, 0.0),
                    ("Purple", 0.8, 0.0, 1.0),
                    ("White", 1.0, 1.0, 1.0),
                ];
                ds_row(ui, |ui| {
                    ds_label(ui, "Presets");
                    ui.horizontal_wrapped(|ui| {
                        for (name, r, g, b) in PRESETS {
                            let active = (self.lightbar.r - r).abs() < 0.01
                                && (self.lightbar.g - g).abs() < 0.01
                                && (self.lightbar.b - b).abs() < 0.01;
                            let col = Color32::from_rgb(
                                (*r * 255.0) as u8,
                                (*g * 255.0) as u8,
                                (*b * 255.0) as u8,
                            );
                            if ds_swatch(ui, col, active).on_hover_text(*name).clicked() {
                                self.lightbar.r = *r;
                                self.lightbar.g = *g;
                                self.lightbar.b = *b;
                                self.apply_lightbar();
                                self.sync_profile();
                            }
                        }
                    });
                });

                ds_row(ui, |ui| {
                    ds_label(ui, "Brightness");
                    if ds_slider(ui, &c, &mut self.lightbar.brightness, 0.0..=255.0).changed() {
                        self.apply_lightbar();
                        self.sync_profile();
                    }
                    ds_value_pct(ui, self.lightbar.brightness / 255.0 * 100.0);
                });

                if self.ipc.is_some() {
                    ds_section(ui, &c, "LED Effects");
                    ds_row(ui, |ui| {
                        ds_label(ui, "Mode");
                        use LightbarEffect::*;
                        let n = matches!(self.lightbar_effect, None);
                        let b = matches!(self.lightbar_effect, Breath { .. });
                        let r = matches!(self.lightbar_effect, Rainbow { .. });
                        let s = matches!(self.lightbar_effect, Strobe { .. });
                        ui.horizontal_wrapped(|ui| {
                            let mut select =
                                |ui: &mut Ui, label: &str, active: bool, new: LightbarEffect| {
                                    if ds_pill_button(ui, &c, label, active).clicked() && !active {
                                        self.lightbar_effect = new;
                                        self.apply_lightbar_effect();
                                        if matches!(self.lightbar_effect, LightbarEffect::None) {
                                            self.apply_lightbar();
                                        }
                                    }
                                };
                            select(ui, "Static", n, None);
                            select(ui, "Breathe", b, Breath { speed: 0.4 });
                            select(ui, "Rainbow", r, Rainbow { speed: 0.15 });
                            select(ui, "Strobe", s, Strobe { speed: 4.0 });
                        });
                    });

                    let mut effect_clone = self.lightbar_effect.clone();
                    let mut effect_changed = false;
                    match &mut effect_clone {
                        LightbarEffect::None => {}
                        LightbarEffect::Breath { speed } => {
                            ds_row(ui, |ui| {
                                ds_label(ui, "Speed");
                                if ds_slider(ui, &c, speed, 0.1..=3.0).changed() {
                                    effect_changed = true;
                                }
                                ds_value_text(
                                    ui,
                                    if *speed < 0.5 {
                                        "slow"
                                    } else if *speed < 1.5 {
                                        "medium"
                                    } else {
                                        "fast"
                                    },
                                );
                            });
                        }
                        LightbarEffect::Rainbow { speed } => {
                            ds_row(ui, |ui| {
                                ds_label(ui, "Speed");
                                if ds_slider(ui, &c, speed, 0.05..=1.0).changed() {
                                    effect_changed = true;
                                }
                                ds_value_text(
                                    ui,
                                    if *speed < 0.2 {
                                        "slow"
                                    } else if *speed < 0.5 {
                                        "medium"
                                    } else {
                                        "fast"
                                    },
                                );
                            });
                        }
                        LightbarEffect::Strobe { speed } => {
                            ds_row(ui, |ui| {
                                ds_label(ui, "Speed");
                                if ds_slider(ui, &c, speed, 1.0..=20.0).changed() {
                                    effect_changed = true;
                                }
                                ds_value_text(ui, &format!("{:.0} Hz", speed));
                            });
                        }
                    }
                    if effect_changed {
                        self.lightbar_effect = effect_clone;
                        self.apply_lightbar_effect();
                    }
                }

                ds_section(ui, &c, "Player Indicator");
                ds_row(ui, |ui| {
                    ds_label(ui, "Player");
                    ui.horizontal_wrapped(|ui| {
                        for i in 0..=7u8 {
                            let active = self.player_leds == i;
                            if ds_pill_button(ui, &c, &format!("{}", i + 1), active).clicked()
                                && !active
                            {
                                self.player_leds = i;
                                self.apply_player_leds();
                                self.sync_profile();
                            }
                        }
                    });
                });
            });
    }
}
