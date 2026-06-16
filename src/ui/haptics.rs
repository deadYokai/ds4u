use egui::{Color32, CornerRadius, Pos2, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::common::HapticPattern;
use crate::theme::ThemeColors;
use crate::ui::widgets::{ds_pill_button, ds_slider};

use super::widgets::{ds_label, ds_row, ds_section, ds_slider_int, ds_value_pct};

const PATTERNS: &[(HapticPattern, &str)] = &[
    (HapticPattern::None, "Off"),
    (HapticPattern::Constant, "Constant"),
    (HapticPattern::Pulse, "Pulse"),
    (HapticPattern::Ramp, "Ramp"),
    (HapticPattern::Wave, "Wave"),
];

fn haptic_intensity(pattern: HapticPattern, strength: u8, speed: f32, elapsed: f32) -> f32 {
    let s = (strength.min(7) as f32 / 7.0).clamp(0.0, 1.0);
    match pattern {
        HapticPattern::None => 0.0,
        HapticPattern::Constant => s,
        HapticPattern::Pulse => {
            let phase = (elapsed * speed).rem_euclid(1.0);
            if phase < 0.5 { s } else { 0.0 }
        }
        HapticPattern::Ramp => {
            let phase = (elapsed * speed).rem_euclid(1.0);
            s * phase
        }
        HapticPattern::Wave => {
            let v = (elapsed * speed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            s * v
        }
    }
}

impl DS4UApp {
    fn render_haptic_visual(
        ui: &mut Ui,
        pattern: HapticPattern,
        strength: u8,
        speed: f32,
        time: f32,
        c: &ThemeColors,
    ) {
        let w = 520.0;
        let h = 110.0;
        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let painter = ui.painter();
        let rounding = CornerRadius::same(6);

        painter.rect_filled(rect, rounding, c.extreme_bg());
        painter.rect_stroke(
            rect,
            rounding,
            Stroke::new(1.0, Color32::WHITE),
            StrokeKind::Outside,
        );

        let pad = 8.0;
        let bottom = rect.max.y - pad;
        let top = rect.min.y + pad;
        let left = rect.min.x + pad;
        let right = rect.max.x - pad;
        let plot_w = right - left;
        let plot_h = bottom - top;

        for frac in [0.25, 0.5, 0.75] {
            let y = bottom - frac * plot_h;
            painter.line_segment(
                [pos2(left, y), pos2(right, y)],
                Stroke::new(0.4, c.widget_inactive()),
            );
        }

        let window: f32 = 3.0;
        let steps = 256usize;
        let mut points: Vec<Pos2> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let elapsed = (time - (1.0 - t) * window).max(0.0);
            let val = haptic_intensity(pattern, strength, speed, elapsed);
            let x = left + t * plot_w;
            let y = bottom - val * plot_h;
            points.push(pos2(x, y));
        }
        painter.add(egui::Shape::line(points, Stroke::new(2.0, c.accent())));

        painter.line_segment(
            [pos2(right, top), pos2(right, bottom)],
            Stroke::new(0.6, c.text_dim()),
        );
        let current = haptic_intensity(pattern, strength, speed, time);
        let dot = pos2(right, bottom - current * plot_h);
        painter.circle_filled(dot, 5.0, c.accent());
        painter.circle_stroke(dot, 5.0, Stroke::new(1.0, Color32::WHITE));
    }

    pub(crate) fn render_haptics_settings(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        let in_daemon = self.ipc.is_some();
        let can_stream = self.controller.is_some() || self.ipc.is_some();
        let mut changed = false;
        let mut pat_changed = false;
        let mut params_changed = false;
        let mut raw_action: Option<bool> = None;
        let mut live_update = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let streaming = self.haptic_stream_active();

                ds_section(ui, &c, "Pattern");
                ds_row(ui, |ui| {
                    ds_label(ui, "Effect");
                    ui.horizontal_wrapped(|ui| {
                        for (p, label) in PATTERNS {
                            let active = self.haptic_state.pattern == *p;
                            if ds_pill_button(ui, &c, label, active).clicked() && !active {
                                self.haptic_state.pattern = *p;
                                if in_daemon {
                                    pat_changed = true;
                                }
                                live_update = streaming;
                            }
                        }
                    });
                });

                if !matches!(self.haptic_state.pattern, HapticPattern::None) {
                    let mut s = self.haptic_state.strength as i32;
                    ds_row(ui, |ui| {
                        ds_label(ui, "Strength");
                        if ds_slider_int(ui, &c, &mut s, 0..=7).changed() {
                            self.haptic_state.strength = s as u8;
                            if in_daemon {
                                params_changed = true;
                            }
                            live_update = streaming;
                        }
                        ds_value_pct(ui, (s as f32 / 7.0) * 100.0);
                    });

                    if !matches!(self.haptic_state.pattern, HapticPattern::Constant) {
                        ds_row(ui, |ui| {
                            ds_label(ui, "Speed");
                            if ds_slider(ui, &c, &mut self.haptic_state.speed, 0.1..=10.0).changed()
                            {
                                if in_daemon {
                                    params_changed = true;
                                }
                                live_update = streaming;
                            }
                            ds_value_pct(ui, (self.haptic_state.speed / 10.0) * 100.0);
                        });
                    }

                    ds_section(ui, &c, "Live preview");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(crate::ui::widgets::ROW_PAD_X);
                        let time = ui.input(|i| i.time) as f32;
                        Self::render_haptic_visual(
                            ui,
                            self.haptic_state.pattern,
                            self.haptic_state.strength,
                            self.haptic_state.speed,
                            time,
                            &c,
                        );
                    });
                }

                ds_section(ui, &c, "Haptics");
                if can_stream {
                    ds_row(ui, |ui| {
                        ds_label(ui, "Stream");
                        let label = if streaming { "Stop" } else { "Start" };
                        if ds_pill_button(ui, &c, label, streaming).clicked() {
                            raw_action = Some(!streaming);
                        }
                    });
                } else {
                    ds_row(ui, |ui| {
                        ds_label(ui, "Status");
                        ui.label(
                            RichText::new("Connect a controller")
                                .size(18.0)
                                .italics()
                                .color(c.text_dim()),
                        );
                    });
                }

                ds_section(ui, &c, "Vibration Attenuation");
                let mut rum = self.vibration.rumble as i32;
                let mut trg = self.vibration.trigger as i32;
                ds_row(ui, |ui| {
                    ds_label(ui, "Rumble");
                    if ds_slider_int(ui, &c, &mut rum, 0..=7).changed() {
                        self.vibration.rumble = rum as u8;
                        changed = true;
                    }
                    ds_value_pct(ui, ((7 - rum) as f32 / 7.0) * 100.0);
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Trigger");
                    if ds_slider_int(ui, &c, &mut trg, 0..=7).changed() {
                        self.vibration.trigger = trg as u8;
                        changed = true;
                    }
                    ds_value_pct(ui, ((7 - trg) as f32 / 7.0) * 100.0);
                });

                ds_section(ui, &c, "Test");
                ds_row(ui, |ui| {
                    ds_label(ui, "Pulse");
                    if ds_pill_button(ui, &c, "Left", false).clicked() {
                        self.test_rumble(255, 0, 350);
                    }
                    ui.add_space(8.0);
                    if ds_pill_button(ui, &c, "Right", false).clicked() {
                        self.test_rumble(0, 255, 350);
                    }
                    ui.add_space(8.0);
                    if ds_pill_button(ui, &c, "Both", false).clicked() {
                        self.test_rumble(255, 255, 350);
                    }
                });
            });

        if pat_changed || params_changed {
            self.apply_haptic_pattern();
            self.sync_profile();
        }
        if changed {
            self.apply_vibration();
        }
        match raw_action {
            Some(true) => self.start_raw_haptics(),
            Some(false) => self.stop_raw_haptics(),
            None => {
                if live_update {
                    self.update_raw_haptics();
                }
            }
        }
    }
}
