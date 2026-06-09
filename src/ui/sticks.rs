use egui::{Align2, Color32, Pos2, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::common::SensitivityCurve;
use crate::theme::ThemeColors;

use super::widgets::{ds_label, ds_row, ds_section, ds_slider, ds_toggle, ds_value_pct};

fn curve_value(curve: &SensitivityCurve, t: f32) -> f32 {
    match curve {
        SensitivityCurve::Default => t,
        SensitivityCurve::Quick => t.powf(0.5),
        SensitivityCurve::Precise => t.powf(2.2),
        SensitivityCurve::Steady => t.powf(1.6),
        SensitivityCurve::Digital => {
            if t > 0.5 {
                1.0
            } else {
                0.0
            }
        }
        SensitivityCurve::Dynamic => {
            let t2 = t * 2.0;
            if t < 0.5 {
                0.5 * t2 * t2
            } else {
                1.0 - 0.5 * (2.0 - t2) * (2.0 - t2)
            }
        }
    }
}

impl DS4UApp {
    fn render_stick_visual(
        ui: &mut Ui,
        deadzone: f32,
        outer: f32,
        raw: Option<[u8; 2]>,
        pressed: bool,
        c: &ThemeColors,
    ) {
        let size = 180.0;
        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();
        let center = rect.center();
        let radius = size * 0.5 - 4.0;

        painter.circle_filled(
            center,
            radius,
            if pressed { c.accent() } else { c.extreme_bg() },
        );
        painter.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));

        if outer < 1.0 {
            painter.circle_stroke(
                center,
                radius * outer.clamp(0.0, 1.0),
                Stroke::new(1.0, c.warning()),
            );
        }

        if deadzone > 0.0 {
            painter.circle_filled(
                center,
                radius * deadzone.clamp(0.0, 1.0),
                Color32::from_rgba_unmultiplied(200, 50, 50, 60),
            );
        }

        if let Some([x, y]) = raw {
            let nx = (x as f32 - 128.0) / 128.0;
            let ny = (y as f32 - 128.0) / 128.0;
            let dot = pos2(
                center.x + nx * (radius - 10.0),
                center.y + ny * (radius - 10.0),
            );
            painter.circle_filled(dot, 8.0, c.accent());
            painter.circle_stroke(dot, 8.0, Stroke::new(1.0, Color32::WHITE));
        }
    }

    fn render_curve_visual(
        ui: &mut Ui,
        curve: &SensitivityCurve,
        deadzone: f32,
        outer: f32,
        c: &ThemeColors,
    ) {
        let size = 140.0;
        let pad = 12.0;

        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();

        painter.rect_filled(rect, 6.0, c.extreme_bg());
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.5, c.widget_inactive()),
            egui::StrokeKind::Outside,
        );

        let plot_rect = egui::Rect::from_min_size(
            pos2(rect.min.x + pad, rect.min.y + pad),
            vec2(size - pad * 2.0, size - pad * 2.0),
        );

        for t in [0.25, 0.5, 0.75] {
            let x = plot_rect.min.x + t * plot_rect.width();
            let y = plot_rect.min.y + t * plot_rect.height();
            painter.line_segment(
                [pos2(x, plot_rect.min.y), pos2(x, plot_rect.max.y)],
                egui::Stroke::new(0.5, c.widget_inactive()),
            );
            painter.line_segment(
                [pos2(plot_rect.min.x, y), pos2(plot_rect.max.x, y)],
                egui::Stroke::new(0.5, c.widget_inactive()),
            );
        }

        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_top()],
            egui::Stroke::new(1.0, c.widget_inactive()),
        );

        let dz_x = plot_rect.min.x + deadzone.clamp(0.0, 1.0) * plot_rect.width();
        painter.rect_filled(
            egui::Rect::from_min_max(plot_rect.min, pos2(dz_x, plot_rect.max.y)),
            0.0,
            Color32::from_rgba_unmultiplied(200, 50, 50, 25),
        );

        let outer_x = plot_rect.min.x + outer.clamp(0.0, 1.0) * plot_rect.width();
        painter.rect_filled(
            egui::Rect::from_min_max(pos2(outer_x, plot_rect.min.y), plot_rect.max),
            0.0,
            Color32::from_rgba_unmultiplied(200, 150, 50, 25),
        );

        let steps = 80;
        let mut points: Vec<Pos2> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let out = curve_value(curve, t);
            let x = plot_rect.min.x + t * plot_rect.width();
            let y = plot_rect.max.y - out * plot_rect.height();
            points.push(pos2(x, y));
        }

        for w in points.windows(2) {
            painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, c.accent()));
        }

        let font = egui::FontId::proportional(9.0);
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 3.0),
            Align2::RIGHT_TOP,
            "0",
            font.clone(),
            c.text_dim(),
        );
        painter.text(
            plot_rect.left_bottom() + vec2(2.0, 3.0),
            Align2::LEFT_TOP,
            "1",
            font.clone(),
            c.text_dim(),
        );
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 0.0),
            Align2::RIGHT_CENTER,
            "1",
            font.clone(),
            c.text_dim(),
        );
    }

    fn curve_combo(ui: &mut Ui, id: &str, value: &mut SensitivityCurve) -> bool {
        let mut changed = false;
        egui::ComboBox::from_id_salt(id)
            .selected_text(format!("{:?}", value))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                for c in [
                    SensitivityCurve::Default,
                    SensitivityCurve::Quick,
                    SensitivityCurve::Precise,
                    SensitivityCurve::Steady,
                    SensitivityCurve::Dynamic,
                    SensitivityCurve::Digital,
                ] {
                    let label = format!("{:?}", c);
                    if ui.selectable_value(value, c, label).changed() {
                        changed = true;
                    }
                }
            });
        changed
    }

    pub(crate) fn render_sticks_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        let mut any_changed = false;
        let left_raw = self
            .input
            .controller_state
            .as_ref()
            .map(|s| [s.left_x, s.left_y]);
        let right_raw = self
            .input
            .controller_state
            .as_ref()
            .map(|s| [s.right_x, s.right_y]);
        let l3 = self
            .input
            .controller_state
            .as_ref()
            .is_some_and(|s| s.buttons & crate::inputs::BTN_L3 != 0);
        let r3 = self
            .input
            .controller_state
            .as_ref()
            .is_some_and(|s| s.buttons & crate::inputs::BTN_R3 != 0);

        let total = ui.available_width();
        let set_w = 620.0_f32.min(total * 0.45);
        let cur_w = 480.0_f32.min((total - set_w) * 0.55);
        let prv_w = (total - set_w - cur_w).max(280.0);

        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                vec2(set_w, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    egui::ScrollArea::vertical()
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ds_section(ui, &c, "Analog Sticks");

                            ds_row(ui, |ui| {
                                ds_label(ui, "L Deadzone");
                                if ds_slider(ui, &c, &mut self.sticks.left_deadzone, 0.0..=0.5)
                                    .changed()
                                {
                                    any_changed = true;
                                }
                                ds_value_pct(ui, self.sticks.left_deadzone * 100.0);
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "R Deadzone");
                                if ds_slider(ui, &c, &mut self.sticks.right_deadzone, 0.0..=0.5)
                                    .changed()
                                {
                                    any_changed = true;
                                }
                                ds_value_pct(ui, self.sticks.right_deadzone * 100.0);
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "L Outer");
                                if ds_slider(
                                    ui,
                                    &c,
                                    &mut self.sticks.left_outer_deadzone,
                                    0.5..=1.0,
                                )
                                .changed()
                                {
                                    any_changed = true;
                                }
                                ds_value_pct(ui, self.sticks.left_outer_deadzone * 100.0);
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "R Outer");
                                if ds_slider(
                                    ui,
                                    &c,
                                    &mut self.sticks.right_outer_deadzone,
                                    0.5..=1.0,
                                )
                                .changed()
                                {
                                    any_changed = true;
                                }
                                ds_value_pct(ui, self.sticks.right_outer_deadzone * 100.0);
                            });

                            ds_section(ui, &c, "Curves");
                            ds_row(ui, |ui| {
                                ds_label(ui, "L Curve");
                                if Self::curve_combo(ui, "left_curve", &mut self.sticks.left_curve)
                                {
                                    any_changed = true;
                                }
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "R Curve");
                                if Self::curve_combo(
                                    ui,
                                    "right_curve",
                                    &mut self.sticks.right_curve,
                                ) {
                                    any_changed = true;
                                }
                            });

                            ds_section(ui, &c, "Options");
                            ds_row(ui, |ui| {
                                ds_label(ui, "Swap L / R");
                                if ds_toggle(ui, &c, &mut self.sticks.swap).changed() {
                                    any_changed = true;
                                }
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "Invert L X");
                                if ds_toggle(ui, &c, &mut self.sticks.left_invert_x).changed() {
                                    any_changed = true;
                                }
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "Invert L Y");
                                if ds_toggle(ui, &c, &mut self.sticks.left_invert_y).changed() {
                                    any_changed = true;
                                }
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "Invert R X");
                                if ds_toggle(ui, &c, &mut self.sticks.right_invert_x).changed() {
                                    any_changed = true;
                                }
                            });
                            ds_row(ui, |ui| {
                                ds_label(ui, "Invert R Y");
                                if ds_toggle(ui, &c, &mut self.sticks.right_invert_y).changed() {
                                    any_changed = true;
                                }
                            });
                        });
                },
            );
            let (s, _) = ui.allocate_exact_size(vec2(1.0, ui.available_height()), Sense::hover());
            ui.painter()
                .rect_filled(s, 0.0, crate::ui::widgets::sep_color(&c));

            ui.allocate_ui_with_layout(
                vec2(cur_w, ui.available_height()),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("RESPONSE CURVE")
                                .size(15.0)
                                .strong()
                                .color(c.accent())
                                .extra_letter_spacing(2.0),
                        );
                    });
                    ui.add_space(10.0);
                    Self::render_curve_visual(
                        ui,
                        &self.sticks.left_curve,
                        self.sticks.left_deadzone,
                        self.sticks.left_outer_deadzone,
                        &c,
                    );
                },
            );

            let (s, _) = ui.allocate_exact_size(vec2(1.0, ui.available_height()), Sense::hover());
            ui.painter()
                .rect_filled(s, 0.0, crate::ui::widgets::sep_color(&c));

            ui.allocate_ui_with_layout(
                vec2(prv_w, ui.available_height()),
                egui::Layout::top_down(egui::Align::Center),
                |ui| {
                    ui.add_space(14.0);
                    ui.label(
                        RichText::new("LIVE PREVIEW")
                            .size(15.0)
                            .strong()
                            .color(c.accent())
                            .extra_letter_spacing(2.0),
                    );
                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        Self::render_stick_visual(
                            ui,
                            self.sticks.left_deadzone,
                            self.sticks.left_outer_deadzone,
                            left_raw,
                            l3,
                            &c,
                        );
                        ui.add_space(20.0);
                        Self::render_stick_visual(
                            ui,
                            self.sticks.right_deadzone,
                            self.sticks.right_outer_deadzone,
                            right_raw,
                            r3,
                            &c,
                        );
                    });
                },
            );
        });

        if any_changed {
            self.apply_input_transform();
            self.sync_profile();
        }
    }
}
