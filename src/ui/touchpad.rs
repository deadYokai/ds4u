use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::common::TouchpadMode;
use crate::inputs::{BTN_TOUCHPAD, TOUCHPAD_MAX_X, TOUCHPAD_MAX_Y};
use crate::theme::ThemeColors;
use crate::ui::widgets::{
    ROW_PAD_X, ds_label, ds_pill_button, ds_row, ds_section, ds_slider, ds_toggle,
};

impl DS4UApp {
    fn render_touchpad_visual(ui: &mut Ui, app: &DS4UApp, c: &ThemeColors) {
        let aspect = TOUCHPAD_MAX_X as f32 / TOUCHPAD_MAX_Y as f32;
        let w = 520.0;
        let h = w / aspect;

        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let painter = ui.painter();
        let rounding = CornerRadius::same(6);

        let pressed = app
            .input
            .controller_state
            .as_ref()
            .is_some_and(|s| s.buttons & BTN_TOUCHPAD != 0);

        if pressed {
            painter.rect_filled(
                rect,
                rounding,
                Color32::from_rgba_unmultiplied(90, 160, 255, 50),
            );
            painter.rect_stroke(
                rect,
                rounding,
                Stroke::new(1.5, c.accent()),
                egui::StrokeKind::Outside,
            );
        } else {
            painter.rect_filled(rect, rounding, c.extreme_bg());
            painter.rect_stroke(
                rect,
                rounding,
                Stroke::new(1.0, Color32::WHITE),
                egui::StrokeKind::Outside,
            );
        }

        if let Some(state) = &app.input.controller_state {
            for pt in state.touch_points.iter().filter(|p| p.active) {
                let nx = pt.x as f32 / TOUCHPAD_MAX_X as f32;
                let ny = pt.y as f32 / TOUCHPAD_MAX_Y as f32;
                let p = pos2(rect.min.x + nx * w, rect.min.y + ny * h);
                painter.circle_filled(p, 9.0, c.accent());
                painter.circle_stroke(p, 9.0, Stroke::new(1.0, Color32::WHITE));
                painter.text(
                    p + vec2(12.0, -12.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("#{}", pt.id),
                    egui::FontId::proportional(10.0),
                    c.text_dim(),
                );
            }
        }
    }
    pub(crate) fn render_touchpad_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        let mut changed = false;
        let mut show_only_changed = false;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ds_section(ui, &c, "Touchpad");
                ui.horizontal(|ui| {
                    ui.add_space(ROW_PAD_X);
                    ui.label(
                        RichText::new("Currently no effect")
                            .size(13.0)
                            .italics()
                            .color(c.text_dim()),
                    );
                });
                ui.add_space(2.0);
                ds_row(ui, |ui| {
                    ds_label(ui, "Mode");
                    ui.horizontal_wrapped(|ui| {
                        for (m, label) in &[
                            (TouchpadMode::Mouse, "Mouse"),
                            (TouchpadMode::GesturesOnly, "Gestures only"),
                            (TouchpadMode::PassThrough, "Pass-through"),
                            (TouchpadMode::Disabled, "Disabled"),
                        ] {
                            let active = self.touchpad.mode == *m;
                            if ds_pill_button(ui, &c, label, active).clicked() && !active {
                                self.touchpad.mode = *m;
                                changed = true;
                            }
                        }
                    });
                });

                let interactive = !matches!(self.touchpad.mode, TouchpadMode::Disabled);

                if interactive && matches!(self.touchpad.mode, TouchpadMode::Mouse) {
                    ds_row(ui, |ui| {
                        ds_label(ui, "Sensitivity");
                        if ds_slider(ui, &c, &mut self.touchpad.sensitivity, 0.1..=3.0).changed() {
                            changed = true;
                        }
                        crate::ui::widgets::ds_value_pct(
                            ui,
                            (self.touchpad.sensitivity / 3.0) * 100.0,
                        );
                    });

                    ds_row(ui, |ui| {
                        ds_label(ui, "Tap to click");
                        if ds_toggle(ui, &c, &mut self.touchpad.tap_to_click).changed() {
                            changed = true;
                        }
                    });

                    ds_row(ui, |ui| {
                        ds_label(ui, "Natural scrolling");
                        if ds_toggle(ui, &c, &mut self.touchpad.natural_scrolling).changed() {
                            changed = true;
                        }
                    });
                }

                ds_section(ui, &c, "Display");
                ds_row(ui, |ui| {
                    ds_label(ui, "Show overlay");
                    if ds_toggle(ui, &c, &mut self.touchpad.show_overlay).changed() {
                        show_only_changed = true;
                    }
                });

                ds_section(ui, &c, "Live preview");
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(ROW_PAD_X);
                    if self.input.controller_state.is_some() {
                        Self::render_touchpad_visual(ui, self, &c);
                    } else {
                        ui.label(
                            RichText::new("Waiting for input…")
                                .size(18.0)
                                .color(c.warning()),
                        );
                    }
                });

                if let Some(s) = &self.input.controller_state {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(ROW_PAD_X);
                        let mut line = format!("active: {}", s.touch_count);
                        for (i, pt) in s.touch_points.iter().enumerate() {
                            if pt.active {
                                line.push_str(&format!("  #{}={}:({},{})", i, pt.id, pt.x, pt.y));
                            }
                        }
                        ui.label(
                            RichText::new(line)
                                .size(13.0)
                                .color(c.text_dim())
                                .monospace(),
                        );
                    });
                }
            });

        if changed {
            self.apply_input_transform();
            self.sync_profile();
        } else if show_only_changed {
            self.sync_profile();
        }
    }
}
