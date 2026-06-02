use egui::{
    Button, Color32, CornerRadius, Pos2, RichText, Sense, Slider, Stroke, StrokeKind, Ui, pos2,
    vec2,
};

use crate::app::DS4UApp;
use crate::common::HapticPattern;
use crate::theme::ThemeColors;

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
        for win in points.windows(2) {
            painter.line_segment([win[0], win[1]], Stroke::new(2.0, c.accent()));
        }

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
        ui.heading(RichText::new("Haptics").size(28.0));
        ui.add_space(10.0);

        let c = self.theme.colors.clone();
        ui.label(
            RichText::new("Vibration patterns and rumble attenuation")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        if self.ipc.is_some() {
            ui.label(RichText::new("Pattern").size(18.0).strong());
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                for (p, label) in PATTERNS {
                    let active = self.haptic_state.pattern == *p;
                    let btn = Button::new(RichText::new(*label).size(13.0))
                        .fill(if active {
                            c.accent()
                        } else {
                            c.widget_inactive()
                        })
                        .min_size(vec2(86.0, 32.0));
                    if ui.add(btn).clicked() && !active {
                        self.haptic_state.pattern = *p;
                        self.apply_haptic_pattern();
                        self.sync_profile();
                    }
                }
            });

            if !matches!(self.haptic_state.pattern, HapticPattern::None) {
                ui.add_space(14.0);

                let mut strength_f = self.haptic_state.strength as i32;
                ui.label("Strength (0 = silent · 7 = full)");
                if ui.add(Slider::new(&mut strength_f, 0..=7)).changed() {
                    self.haptic_state.strength = strength_f as u8;
                    self.apply_haptic_pattern();
                    self.sync_profile();
                }

                if !matches!(self.haptic_state.pattern, HapticPattern::Constant) {
                    ui.add_space(8.0);
                    ui.label("Speed (Hz)");
                    if ui
                        .add(Slider::new(&mut self.haptic_state.speed, 0.1..=10.0))
                        .changed()
                    {
                        self.apply_haptic_pattern();
                        self.sync_profile();
                    }
                }
            }

            ui.add_space(18.0);
            ui.label(RichText::new("Live preview").size(14.0).strong());
            ui.add_space(6.0);
            let time = ui.input(|i| i.time) as f32;
            Self::render_haptic_visual(
                ui,
                self.haptic_state.pattern,
                self.haptic_state.strength,
                self.haptic_state.speed,
                time,
                &c,
            );

            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);
        } else {
            ui.label(
                RichText::new("Haptic patterns require the daemon to be running.")
                    .size(12.0)
                    .color(c.text_dim()),
            );
            ui.add_space(20.0);
            ui.separator();
            ui.add_space(20.0);
        }

        ui.label(RichText::new("Vibration Attenuation").size(18.0).strong());
        ui.add_space(6.0);
        ui.label(
            RichText::new("0 = full strength · 7 = quietest")
                .size(12.0)
                .color(c.text_dim()),
        );
        ui.add_space(10.0);

        let mut changed = false;
        ui.horizontal(|ui| {
            ui.label("Rumble motors:");
            if ui
                .add(Slider::new(&mut self.vibration.rumble, 0..=7).text(""))
                .changed()
            {
                changed = true;
            }
            ui.label(format!("{}", self.vibration.rumble));
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Trigger vibration:");
            if ui
                .add(Slider::new(&mut self.vibration.trigger, 0..=7).text(""))
                .changed()
            {
                changed = true;
            }
            ui.label(format!("{}", self.vibration.trigger));
        });

        if changed {
            self.apply_vibration();
        }
    }
}
