use egui::{Color32, Pos2, Rect, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::firmware::get_product_name;
use crate::theme::ThemeColors;
use crate::ui::widgets::{
    ROW_HEIGHT, ds_label, ds_pill_button, ds_row, ds_section, ds_value_text_lr,
};

use super::widgets::{LBL_WIDTH, ds_gradient_line, warning_of, with_alpha};

fn paint_check(p: &egui::Painter, center: Pos2, size: f32, col: Color32) {
    let sw = (size * 0.20).max(2.0);
    let a = center + vec2(-size * 0.55, 0.0);
    let b = center + vec2(-size * 0.10, size * 0.40);
    let c = center + vec2(size * 0.55, -size * 0.40);
    p.line_segment([a, b], Stroke::new(sw, col));
    p.line_segment([b, c], Stroke::new(sw, col));
}

fn paint_cross(p: &egui::Painter, center: Pos2, size: f32, col: Color32) {
    let r = size * 0.42;
    let sw = (size * 0.20).max(2.0);
    p.line_segment(
        [center + vec2(-r, -r), center + vec2(r, r)],
        Stroke::new(sw, col),
    );
    p.line_segment(
        [center + vec2(-r, r), center + vec2(r, -r)],
        Stroke::new(sw, col),
    );
}

fn paint_badge(
    p: &egui::Painter,
    center: Pos2,
    radius: f32,
    fill: Color32,
    glyph_col: Color32,
    glyph: fn(&egui::Painter, Pos2, f32, Color32),
) {
    p.circle_filled(center, radius * 1.45, with_alpha(fill, 22));
    p.circle_filled(center, radius * 1.20, with_alpha(fill, 52));
    p.circle_filled(center, radius, fill);
    p.circle_stroke(center, radius, Stroke::new(1.5, with_alpha(fill, 210)));
    glyph(p, center, radius * 0.95, glyph_col);
}

fn paint_progress(ui: &Ui, c: &ThemeColors, rect: Rect, t: f32) {
    let y = rect.center().y;
    let track = Rect::from_min_max(pos2(rect.min.x, y - 2.0), pos2(rect.max.x, y + 2.0));
    let fill_w = rect.width() * t.clamp(0.0, 1.0);
    let fill = Rect::from_min_max(track.min, pos2(track.min.x + fill_w, track.max.y));
    let badge = warning_of(ui);
    let p = ui.painter();
    p.rect_filled(track, 2.0, with_alpha(badge, 40));
    p.rect_filled(fill, 2.0, c.accent());
}

fn paint_status_dot(ui: &mut Ui, color: Color32, text: &str) {
    let font_size = 18.0;
    let dot_r = 5.5;
    let gap = 8.0;
    let galley =
        ui.painter()
            .layout_no_wrap(text.into(), egui::FontId::proportional(font_size), color);
    let w = dot_r * 2.0 + gap + galley.size().x;
    let (rect, _) = ui.allocate_exact_size(vec2(w, ROW_HEIGHT), Sense::hover());
    let p = ui.painter();
    p.circle_filled(pos2(rect.min.x + dot_r, rect.center().y), dot_r, color);
    p.text(
        pos2(rect.min.x + dot_r * 2.0 + gap, rect.center().y),
        egui::Align2::LEFT_CENTER,
        text,
        egui::FontId::proportional(font_size),
        color,
    );
}

fn badge_label(
    ui: &mut Ui,
    badge_color: Color32,
    glyph_col: Color32,
    glyph: fn(&egui::Painter, Pos2, f32, Color32),
) {
    let (rect, _) = ui.allocate_exact_size(vec2(LBL_WIDTH, ROW_HEIGHT), Sense::hover());
    let center = pos2(rect.min.x + 26.0, rect.center().y);
    paint_badge(ui.painter(), center, 16.0, badge_color, glyph_col, glyph);
}

impl DS4UApp {
    fn render_result_panel(
        &mut self,
        ui: &mut Ui,
        c: &ThemeColors,
        result: Result<(), String>,
        connected: bool,
        is_bt: bool,
    ) {
        ds_gradient_line(ui);

        match result {
            Ok(()) => {
                ds_section(ui, c, "Update Successful");

                ds_row(ui, |ui| {
                    badge_label(ui, c.success(), c.window_bg(), paint_check);
                    ui.label(
                        RichText::new("Firmware Updated")
                            .size(22.0)
                            .strong()
                            .color(c.text()),
                    );
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "");
                    ui.label(
                        RichText::new("Re-read controller info to confirm the new version.")
                            .size(14.0)
                            .color(c.text_dim()),
                    );
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "");
                    if connected && ds_pill_button(ui, c, "↻ Refresh now", false).clicked() {
                        self.refresh_firmware_info();
                        self.firmware.last_flash_result = None;
                    }
                    ui.add_space(8.0);
                    if ds_pill_button(ui, c, "Dismiss", false).clicked() {
                        self.firmware.last_flash_result = None;
                    }
                });
            }
            Err(err) => {
                ds_section(ui, c, "Update Failed");

                ds_row(ui, |ui| {
                    badge_label(ui, c.error(), c.window_bg(), paint_cross);
                    ui.label(
                        RichText::new("Flash failed")
                            .size(22.0)
                            .strong()
                            .color(c.text()),
                    );
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Reason");
                    ui.label(RichText::new(&err).size(15.0).color(c.error()));
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "");
                    ui.label(
                        RichText::new("Reconnect the controller via USB and try again.")
                            .size(14.0)
                            .color(c.text_dim()),
                    );
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "");
                    let can_retry = connected && !is_bt;
                    if can_retry && ds_pill_button(ui, c, "Retry", false).clicked() {
                        self.firmware.last_flash_result = None;
                        self.flash_latest();
                    }
                    ui.add_space(8.0);
                    if ds_pill_button(ui, c, "Dismiss", false).clicked() {
                        self.firmware.last_flash_result = None;
                    }
                });
            }
        }
    }

    fn render_flashing_panel(&mut self, ui: &mut Ui, c: &ThemeColors, status: &str, progress: u32) {
        ds_section(ui, c, "Flashing");

        ui.ctx().request_repaint();

        let status_text = if status.is_empty() {
            "Preparing…".to_string()
        } else {
            status.to_string()
        };

        ds_row(ui, |ui| {
            ds_label(ui, "Step");
            ui.spinner();
            ui.add_space(8.0);
            ui.label(RichText::new(status_text).size(18.0).color(c.text()));
        });

        ds_row(ui, |ui| {
            ds_label(ui, "Progress");
            let pct_w = 64.0;
            let bar_w = (ui.available_width() - pct_w - 12.0).max(60.0);

            let (bar_rect, _) = ui.allocate_exact_size(vec2(bar_w, ROW_HEIGHT), Sense::hover());
            let target = progress as f32 / 100.0;
            let smoothed = ui.ctx().animate_value_with_time(
                egui::Id::new("ds4u_fw_progress_anim"),
                target.clamp(0.0, 1.0),
                0.25,
            );
            paint_progress(ui, c, bar_rect, smoothed);

            ui.add_space(12.0);
            ui.label(
                RichText::new(format!("{:>3}%", (smoothed * 100.0).round() as i32))
                    .size(18.0)
                    .strong()
                    .color(c.text()),
            );
        });

        ds_row(ui, |ui| {
            ds_label(ui, "Notice");
            ui.label(
                RichText::new("Do not disconnect • keep battery ≥ 10% • takes several minutes")
                    .size(15.0)
                    .color(c.warning()),
            );
        });
    }

    fn render_update_panel(&mut self, ui: &mut Ui, c: &ThemeColors, connected: bool, is_bt: bool) {
        ds_section(ui, c, "Update");
        ds_row(ui, |ui| {
            ds_label(ui, "Action");
            let can_flash = connected && !is_bt;
            let ota = ds_pill_button(ui, c, "Download & Update", false);
            ui.add_space(8.0);
            let file = ds_pill_button(ui, c, "Update from File…", false);
            if can_flash && ota.clicked() {
                self.flash_latest();
            }
            if can_flash && file.clicked() {
                self.flash_file();
            }
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Notice");
            ui.label(
                RichText::new(
                    "Do not disconnect during update // battery ≥ 10% // takes several minutes",
                )
                .size(15.0)
                .color(c.warning()),
            );
        });
        if connected && is_bt {
            ds_row(ui, |ui| {
                ds_label(ui, "Bluetooth");
                ui.label(
                    RichText::new("Switch to USB to flash")
                        .size(18.0)
                        .color(c.error()),
                );
            });
        }
    }

    fn render_firmware_panel(&mut self, ui: &mut Ui) {
        let connected = self.is_connected();
        let is_bt = self.controller_is_bt.unwrap_or(false);

        let model = self
            .controller_product_id
            .map(get_product_name)
            .unwrap_or("-");

        let serial = self
            .controller_serial
            .clone()
            .unwrap_or_else(|| "-".to_string());

        let cur_str = self
            .firmware
            .current_version
            .map(|v| format!("0x{:04X}", v))
            .unwrap_or_else(|| {
                if connected {
                    "-".into()
                } else {
                    "Not connected".into()
                }
            });

        let build_date = self.firmware.build_date.clone().unwrap_or("-".into());
        let build_time = self.firmware.build_time.clone().unwrap_or("-".into());

        let latest_str = self.firmware.latest_version.clone();
        let checking = self.firmware.checking_latest;

        let fw_updating = self.firmware.updating;
        let fw_progress = self.firmware.progress;
        let fw_status = self.firmware.status.clone();
        let last_result = self.firmware.last_flash_result.clone();

        let needs_update: Option<bool> =
            if let (Some(cur), Some(latest)) = (self.firmware.current_version, &latest_str) {
                u16::from_str_radix(latest.trim_start_matches("0x"), 16)
                    .ok()
                    .map(|li| li > cur)
            } else {
                None
            };

        let c = self.theme.colors.clone();

        ds_section(ui, &c, "Controller");
        ds_row(ui, |ui| {
            ds_label(ui, "Model");
            ds_value_text_lr(ui, model);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Serial");
            ds_value_text_lr(ui, &serial);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Build Date");
            ds_value_text_lr(ui, &build_date);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Build Time");
            ds_value_text_lr(ui, &build_time);
        });

        ds_section(ui, &c, "Firmware");
        ds_row(ui, |ui| {
            ds_label(ui, "Current");
            ds_value_text_lr(ui, &cur_str);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Latest");
            if checking {
                ui.spinner();
                ui.add_space(6.0);
                ui.label(RichText::new("Checking…").size(18.0).color(c.text_dim()));
            } else if let Some(ver) = latest_str.as_ref() {
                ds_value_text_lr(ui, ver);
            } else if connected {
                if ds_pill_button(ui, &c, "Check", false).clicked() {
                    self.fetch_latest_verision_async();
                }
            } else {
                ds_value_text_lr(ui, "-");
            }
        });
        if connected {
            ds_row(ui, |ui| {
                ds_label(ui, "Refresh");
                if ds_pill_button(ui, &c, "↻ Re-read info", false).clicked() {
                    self.refresh_firmware_info();
                }
            });
        }
        if let Some(nu) = needs_update {
            ds_row(ui, |ui| {
                ds_label(ui, "Status");
                let (txt, col) = if nu {
                    ("Update available", c.warning())
                } else {
                    ("Up to date", c.success())
                };
                paint_status_dot(ui, col, txt);
            });
        }

        if fw_updating {
            self.render_flashing_panel(ui, &c, &fw_status, fw_progress);
        } else if let Some(result) = last_result {
            self.render_result_panel(ui, &c, result, connected, is_bt);
        } else if let Some(true) = needs_update {
            self.render_update_panel(ui, &c, connected, is_bt);
        }
    }

    pub(crate) fn render_advanced(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_firmware_panel(ui);
            });
    }
}
