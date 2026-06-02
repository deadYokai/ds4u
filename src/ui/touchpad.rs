use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::inputs::{BTN_TOUCHPAD, TOUCHPAD_MAX_X, TOUCHPAD_MAX_Y};
use crate::theme::ThemeColors;

impl DS4UApp {
    fn render_touchpad_visual(ui: &mut Ui, app: &DS4UApp, c: &ThemeColors) {
        let aspect = TOUCHPAD_MAX_X as f32 / TOUCHPAD_MAX_Y as f32;
        let w = 520.0;
        let h = w / aspect;

        let (rect, _) = ui.allocate_exact_size(vec2(w, h), Sense::hover());
        let painter = ui.painter();
        let rounding = CornerRadius::same(6);

        let pressed = app
            .controller_state
            .as_ref()
            .map_or(false, |s| s.buttons & BTN_TOUCHPAD != 0);

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

        if let Some(state) = &app.controller_state {
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
        ui.heading(RichText::new("Touchpad").size(28.0));
        ui.add_space(10.0);

        let c = self.theme.colors.clone();
        ui.label(
            RichText::new("Touchpad behaviour and live preview")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        let mut changed = false;

        if ui
            .checkbox(&mut self.touchpad.enabled, "Enable touchpad input")
            .on_hover_text("When disabled, touch points are stripped from the IPC input stream.")
            .changed()
        {
            changed = true;
        }

        ui.add_space(6.0);

        if ui
            .checkbox(
                &mut self.touchpad.show_overlay,
                "Show overlay on Inputs page",
            )
            .changed()
        {
            self.sync_profile();
        }

        ui.add_space(20.0);
        ui.label(RichText::new("Live preview").size(14.0).strong());
        ui.add_space(6.0);

        if self.controller_state.is_some() {
            Self::render_touchpad_visual(ui, self, &c);
            ui.add_space(10.0);
            let s = self.controller_state.as_ref().unwrap();
            let mut line = format!("active: {}", s.touch_count);
            for (i, pt) in s.touch_points.iter().enumerate() {
                if pt.active {
                    line.push_str(&format!("  #{}={}:({},{})", i, pt.id, pt.x, pt.y));
                }
            }
            ui.label(
                RichText::new(line)
                    .size(11.0)
                    .color(c.text_dim())
                    .monospace(),
            );
        } else {
            ui.label(
                RichText::new("Waiting for input from controller...")
                    .size(12.0)
                    .color(c.warning()),
            );
        }

        if changed {
            self.apply_input_transform();
            self.sync_profile();
        }
    }
}
