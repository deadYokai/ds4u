use egui::{RichText, Ui, vec2};

use crate::app::DS4UApp;
use crate::common::TriggerMode;
use crate::profiles::TriggerConfig;
use crate::ui::widgets::{ds_label, ds_row, ds_slider_int, ds_value_text};

use super::widgets::{ds_pill_button, ds_section};

const ALL_MODES: &[TriggerMode] = &[
    TriggerMode::Off,
    TriggerMode::Feedback,
    TriggerMode::Weapon,
    TriggerMode::Bow,
    TriggerMode::Galloping,
    TriggerMode::Vibration,
    TriggerMode::Machine,
];

fn mode_label(m: &TriggerMode) -> &'static str {
    match m {
        TriggerMode::Off => "Off",
        TriggerMode::Feedback => "Feedback",
        TriggerMode::Weapon => "Weapon",
        TriggerMode::Bow => "Bow",
        TriggerMode::Galloping => "Galloping",
        TriggerMode::Vibration => "Vibration",
        TriggerMode::Machine => "Machine",
    }
}

impl DS4UApp {
    fn render_trigger_panel(
        ui: &mut Ui,
        c: &crate::theme::ThemeColors,
        label: &str,
        cfg: &mut TriggerConfig,
    ) -> bool {
        let mut changed = false;

        ds_section(ui, c, label);

        ds_row(ui, |ui| {
            ds_label(ui, "Mode");
            ui.horizontal_wrapped(|ui| {
                for m in ALL_MODES {
                    let active = std::mem::discriminant(&cfg.mode) == std::mem::discriminant(m);
                    if ds_pill_button(ui, c, mode_label(m), active).clicked() && !active {
                        cfg.mode = m.clone();
                        changed = true;
                    }
                }
            });
        });

        match cfg.mode {
            TriggerMode::Off => {
                ds_row(ui, |ui| {
                    ds_label(ui, "Status");
                    ui.label(
                        RichText::new("Effect disabled")
                            .size(18.0)
                            .italics()
                            .color(c.text_dim()),
                    );
                });
            }
            TriggerMode::Feedback => {
                let mut s = cfg.start as i32;
                let mut e = cfg.end as i32;
                let mut g = cfg.strength as i32;
                ds_row(ui, |ui| {
                    ds_label(ui, "Start");
                    if ds_slider_int(ui, c, &mut s, 0..=9).changed() {
                        cfg.start = s as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &s.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "End");
                    if ds_slider_int(ui, c, &mut e, 0..=9).changed() {
                        cfg.end = e as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &e.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Strength");
                    if ds_slider_int(ui, c, &mut g, 1..=8).changed() {
                        cfg.strength = g as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &g.to_string());
                });
            }
            TriggerMode::Weapon => {
                let lo = cfg.start as i32 + 1;
                let mut s = cfg.start as i32;
                let mut e = cfg.end as i32;
                let mut g = cfg.strength as i32;
                ds_row(ui, |ui| {
                    ds_label(ui, "Pre-pull");
                    if ds_slider_int(ui, c, &mut s, 2..=7).changed() {
                        cfg.start = s as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &s.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Break");
                    if ds_slider_int(ui, c, &mut e, lo..=8).changed() {
                        cfg.end = e as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &e.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Strength");
                    if ds_slider_int(ui, c, &mut g, 1..=8).changed() {
                        cfg.strength = g as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &g.to_string());
                });
            }
            TriggerMode::Bow => {
                let lo = cfg.start as i32 + 1;
                let mut s = cfg.start as i32;
                let mut e = cfg.end as i32;
                let mut g = cfg.strength as i32;
                ds_row(ui, |ui| {
                    ds_label(ui, "Start");
                    if ds_slider_int(ui, c, &mut s, 0..=8).changed() {
                        cfg.start = s as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &s.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Snap");
                    if ds_slider_int(ui, c, &mut e, lo..=8).changed() {
                        cfg.end = e as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &e.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Force");
                    if ds_slider_int(ui, c, &mut g, 1..=8).changed() {
                        cfg.strength = g as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &g.to_string());
                });
            }
            TriggerMode::Galloping | TriggerMode::Vibration | TriggerMode::Machine => {
                let lo = cfg.start as i32 + 1;
                let mut s = cfg.start as i32;
                let mut e = cfg.end as i32;
                let mut g = cfg.strength as i32;
                let mut f = cfg.frequency as i32;
                ds_row(ui, |ui| {
                    ds_label(ui, "Start");
                    if ds_slider_int(ui, c, &mut s, 0..=8).changed() {
                        cfg.start = s as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &s.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "End");
                    if ds_slider_int(ui, c, &mut e, lo..=9).changed() {
                        cfg.end = e as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &e.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Amplitude");
                    if ds_slider_int(ui, c, &mut g, 1..=8).changed() {
                        cfg.strength = g as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &g.to_string());
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Frequency");
                    if ds_slider_int(ui, c, &mut f, 1..=255).changed() {
                        cfg.frequency = f as u8;
                        changed = true;
                    }
                    ds_value_text(ui, &format!("{} Hz", f));
                });
            }
        }

        ds_section(ui, c, "Deadband");
        let lo = cfg.deadband.release as i32 + 1;
        let mut rel = cfg.deadband.release as i32;
        let mut full = cfg.deadband.full_stroke as i32;
        ds_row(ui, |ui| {
            ds_label(ui, "Release");
            if ds_slider_int(ui, c, &mut rel, 0..=254).changed() {
                cfg.deadband.release = rel as u8;
                changed = true;
            }
            ds_value_text(ui, &rel.to_string());
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Full");
            if ds_slider_int(ui, c, &mut full, lo..=255).changed() {
                cfg.deadband.full_stroke = full as u8;
                changed = true;
            }
            ds_value_text(ui, &full.to_string());
        });

        changed
    }

    pub(crate) fn render_triggers_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        let mut left_changed = false;
        let mut right_changed = false;

        let (mut l, mut r) = (self.triggers.left.clone(), self.triggers.right.clone());

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let total = ui.available_width();
                let gutter = 16.0;
                let col_w = ((total - gutter) * 0.5).max(160.0);

                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;

                    let left = ui
                        .allocate_ui_with_layout(
                            vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                left_changed =
                                    Self::render_trigger_panel(ui, &c, "L2 — Left Trigger", &mut l);
                            },
                        )
                        .response
                        .rect;

                    ui.add_space(gutter);

                    let right = ui
                        .allocate_ui_with_layout(
                            vec2(col_w, 0.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                right_changed = Self::render_trigger_panel(
                                    ui,
                                    &c,
                                    "R2 — Right Trigger",
                                    &mut r,
                                );
                            },
                        )
                        .response
                        .rect;

                    let sep_x = (left.max.x + right.min.x) * 0.5;
                    let max_y = left.max.y.max(right.max.y);
                    ui.painter().line_segment(
                        [egui::pos2(sep_x, left.min.y), egui::pos2(sep_x, max_y)],
                        egui::Stroke::new(1.0, crate::ui::widgets::sep_color(&c)),
                    );
                });

                ds_section(ui, &c, "Actions");
                ds_row(ui, |ui| {
                    ds_label(ui, "Reset");
                    if ds_pill_button(ui, &c, "Both triggers to Off", false).clicked() {
                        self.triggers.left = TriggerConfig::default();
                        self.triggers.right = TriggerConfig::default();
                        self.apply_triggers();
                        self.apply_input_transform();
                        self.sync_profile();
                    }
                });
            });

        self.triggers.left = l;
        self.triggers.right = r;

        if left_changed || right_changed {
            self.apply_triggers();
            self.apply_input_transform();
            self.sync_profile();
        }
    }
}
