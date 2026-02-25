use egui::{pos2, vec2, Align2, Color32, Pos2, RichText, Sense, Slider, Ui};

use crate::app::DS4UApp;
use crate::common::SensitivityCurve;

fn curve_value(curve: &SensitivityCurve, t: f32) -> f32 {
    match curve {
        SensitivityCurve::Default => t,
        SensitivityCurve::Quick   => t.powf(0.5),
        SensitivityCurve::Precise => t.powf(2.2),
        SensitivityCurve::Steady  => t.powf(1.6),
        SensitivityCurve::Digital => if t > 0.5 { 1.0 } else { 0.0 },
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
    fn render_stick_visual(ui: &mut Ui, deadzone: f32) {
        let size = 120.0;
        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();
        let center = rect.center();
        let radius = size / 2.0;

        painter.circle_stroke(
            center,
            radius - 1.0,
            egui::Stroke::new(2.0, Color32::from_rgb(50, 70, 100))
        );

        painter.circle_filled(
            center,
            radius - 2.0,
            Color32::from_rgb(12, 18, 30)
        );

        let dz_radius = deadzone / 0.3 * (radius - 4.0);

        painter.circle_filled(
            center,
            dz_radius,
            Color32::from_rgba_unmultiplied(220, 60, 60, 40)
        );

        painter.circle_stroke(
            center,
            dz_radius,
            egui::Stroke::new(1.0, Color32::from_rgb(200, 60, 60))
        );

        painter.circle_filled(
            center,
            4.0,
            Color32::from_rgb(0, 122, 250)
        );
    }

    fn render_curve_visual(ui: &mut Ui, curve: &SensitivityCurve, deadzone: f32) {
        let size = 140.0;
        let pad = 12.0;

        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();

        painter.rect_filled(
            rect,
            6.0,
            Color32::from_rgb(10, 16, 26)
        );

        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.5, Color32::from_rgb(40, 60, 90)),
            egui::StrokeKind::Outside
        );

        let plot_rect = egui::Rect::from_min_size(
            pos2(rect.min.x + pad, rect.min.y + pad),
            vec2(size - pad * 2.0, size - pad * 2.0)
        );

        for t in [0.25, 0.5, 0.75] {
            let x = plot_rect.min.x + t * plot_rect.width();
            let y = plot_rect.min.y + t * plot_rect.height();

            painter.line_segment(
                [pos2(x, plot_rect.min.y), pos2(x, plot_rect.max.y)],
                egui::Stroke::new(0.5, Color32::from_rgb(25, 40, 60))
            );

            painter.line_segment(
                [pos2(plot_rect.min.x, y), pos2(plot_rect.max.x, y)],
                egui::Stroke::new(0.5, Color32::from_rgb(25, 40, 60))
            );
        }

        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_top()],
            egui::Stroke::new(1.0, Color32::from_rgb(40, 60, 80))
        );

        let dz_x = plot_rect.min.x + deadzone / 0.3 * plot_rect.width() * 0.3;

        painter.rect_filled(
            egui::Rect::from_min_max(
                plot_rect.min,
                pos2(dz_x, plot_rect.max.y)
            ),
            0.0,
            Color32::from_rgba_unmultiplied(200, 50, 50, 25)
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

        let accent = Color32::from_rgb(0, 150, 255);
        for w in points.windows(2) {
            painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, accent));
        }

        let font = egui::FontId::proportional(9.0);
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 3.0),
            Align2::RIGHT_TOP, "0",
            font.clone(),
            Color32::from_gray(80)
        );
        painter.text(
            plot_rect.left_bottom() + vec2(2.0, 3.0),
            Align2::LEFT_TOP, "1",
            font.clone(),
            Color32::from_gray(80)
        );
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 0.0),
            Align2::RIGHT_CENTER, "1",
            font.clone(),
            Color32::from_gray(80)
        );
    }

    pub(crate) fn render_sticks_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Stick Sensitivity").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Adjust response curves and deadzones")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.columns(2, |cols| {
            cols[0].label(RichText::new("Left Stick").size(16.0).strong());
            cols[0].add_space(10.0);

            egui::ComboBox::from_id_salt("left_curve")
                .selected_text(format!("{:?}", self.sticks.left_curve))
                .width(cols[0].available_width())
                .show_ui(&mut cols[0], |ui| {
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Default,
                        "Default"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Quick,
                        "Quick"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Precise,
                        "Precise"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Steady,
                        "Steady"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Dynamic,
                        "Dynamic"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Digital,
                        "Digital"
                    );
                });

            Self::render_curve_visual(
                &mut cols[0],
                &self.sticks.left_curve,
                self.sticks.left_deadzone
            );

            cols[0].add_space(15.0);
            cols[0].label("Deadzone");
            if cols[0].add(Slider::new(&mut self.sticks.left_deadzone, 0.0..=0.3))
                .changed()
            {
                self.apply_input_transform();
            }
            Self::render_stick_visual(&mut cols[0], self.sticks.left_deadzone);

            cols[1].label(RichText::new("Right Stick").size(16.0).strong());
            cols[1].add_space(10.0);

            egui::ComboBox::from_id_salt("right_curve")
                .selected_text(format!("{:?}", self.sticks.right_curve))
                .width(cols[0].available_width())
                .show_ui(&mut cols[1], |ui| {
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Default,
                        "Default"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Quick,
                        "Quick"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Precise,
                        "Precise"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Steady,
                        "Steady"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Dynamic,
                        "Dynamic"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Digital,
                        "Digital"
                    );
                });

            Self::render_curve_visual(
                &mut cols[1],
                &self.sticks.right_curve,
                self.sticks.right_deadzone
            );

            cols[1].add_space(15.0);
            cols[1].label("Deadzone");
            if cols[1].add(Slider::new(&mut self.sticks.right_deadzone, 0.0..=0.3))
                .changed()
            {
                self.apply_input_transform();
            }
            Self::render_stick_visual(&mut cols[1], self.sticks.right_deadzone);
        });
    }
}
