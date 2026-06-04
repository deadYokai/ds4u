use egui::{Color32, RichText, Sense, Slider, Stroke, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::theme::ThemeColors;

impl DS4UApp {
    fn render_gyro_visual(ui: &mut Ui, gyro: [i16; 3], accel: [i16; 3], c: &ThemeColors) {
        ui.horizontal(|ui| {
            let size = 160.0;
            let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
            let p = ui.painter();
            let center = rect.center();
            let radius = size * 0.5 - 4.0;

            p.circle_filled(center, radius, c.extreme_bg());
            p.circle_stroke(center, radius, Stroke::new(1.0, Color32::WHITE));
            p.line_segment(
                [
                    pos2(center.x - radius, center.y),
                    pos2(center.x + radius, center.y),
                ],
                Stroke::new(0.5, c.widget_inactive()),
            );
            p.line_segment(
                [
                    pos2(center.x, center.y - radius),
                    pos2(center.x, center.y + radius),
                ],
                Stroke::new(0.5, c.widget_inactive()),
            );

            let ax = (accel[0] as f32 / 8192.0).clamp(-1.0, 1.0);
            let az = (accel[2] as f32 / 8192.0).clamp(-1.0, 1.0);
            let dot = pos2(
                center.x + ax * (radius - 10.0),
                center.y + az * (radius - 10.0),
            );
            p.circle_filled(dot, 8.0, c.accent());
            p.circle_stroke(dot, 8.0, Stroke::new(1.0, Color32::WHITE));

            ui.add_space(12.0);

            ui.vertical(|ui| {
                let bar_w = 260.0;
                let bar_h = 18.0;
                let labels = ["X (pitch)", "Y (yaw)", "Z (roll)"];
                for (i, &val) in gyro.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(labels[i]).size(11.0).color(c.text_dim()));
                        let (bar_rect, _) =
                            ui.allocate_exact_size(vec2(bar_w, bar_h), Sense::hover());
                        let p = ui.painter();
                        p.rect_filled(bar_rect, 3.0, c.extreme_bg());
                        p.rect_stroke(
                            bar_rect,
                            3.0,
                            Stroke::new(1.0, Color32::WHITE),
                            egui::StrokeKind::Outside,
                        );
                        let center_x = bar_rect.center().x;
                        p.line_segment(
                            [
                                pos2(center_x, bar_rect.min.y),
                                pos2(center_x, bar_rect.max.y),
                            ],
                            Stroke::new(0.5, c.text_dim()),
                        );
                        let n = (val as f32 / 32768.0).clamp(-1.0, 1.0);
                        let half = bar_rect.width() * 0.5;
                        let fill = if n >= 0.0 {
                            egui::Rect::from_min_max(
                                pos2(center_x, bar_rect.min.y + 2.0),
                                pos2(center_x + half * n, bar_rect.max.y - 2.0),
                            )
                        } else {
                            egui::Rect::from_min_max(
                                pos2(center_x + half * n, bar_rect.min.y + 2.0),
                                pos2(center_x, bar_rect.max.y - 2.0),
                            )
                        };
                        p.rect_filled(fill, 2.0, c.accent());
                    });
                }
            });
        });
    }

    pub(crate) fn render_gyroscope_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Gyroscope").size(28.0));
        ui.add_space(10.0);

        let c = self.theme.colors.clone();
        ui.label(
            RichText::new("Motion sensor toggle, sensitivity and smoothing")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        let mut changed = false;

        if ui
            .checkbox(&mut self.gyro.processor.enabled, "Enable gyroscope")
            .changed()
        {
            changed = true;
        }

        if self.gyro.processor.enabled {
            ui.add_space(15.0);

            ui.label("Sensitivity");
            if ui
                .add(Slider::new(&mut self.gyro.processor.sensitivity, 0.0..=4.0).text("×"))
                .changed()
            {
                changed = true;
            }

            ui.add_space(10.0);

            ui.label("Smoothing (EMA factor, higher = smoother but laggier)");
            if ui
                .add(Slider::new(&mut self.gyro.processor.smoothing, 0.0..=0.95))
                .changed()
            {
                changed = true;
            }

            ui.add_space(20.0);

            ui.label(RichText::new("Live readout").size(14.0).strong());
            ui.add_space(6.0);

            match self.input.controller_state.as_ref() {
                Some(s) => {
                    Self::render_gyro_visual(ui, s.gyro, s.accel, &c);
                    ui.add_space(6.0);
                    ui.label(
                        RichText::new(format!(
                            "gyro: x={:>6} y={:>6} z={:>6}    accel: x={:>6} y={:>6} z={:>6}",
                            s.gyro[0], s.gyro[1], s.gyro[2], s.accel[0], s.accel[1], s.accel[2]
                        ))
                        .size(11.0)
                        .color(c.text_dim())
                        .monospace(),
                    );
                }
                None => {
                    ui.label(
                        RichText::new("Waiting for input from controller...")
                            .size(12.0)
                            .color(c.warning()),
                    );
                }
            }
        } else {
            ui.add_space(10.0);
            ui.label(
                RichText::new("Gyroscope disabled - axes are zeroed in the IPC stream.")
                    .size(12.0)
                    .color(c.text_dim()),
            );
        }

        if changed {
            self.apply_gyro();
            self.sync_profile();
        }
    }
}
