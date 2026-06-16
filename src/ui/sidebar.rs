use egui::{Color32, CornerRadius, Rect, RichText, Sense, Stroke, StrokeKind, Ui, pos2, vec2};

use crate::app::DS4UApp;
use crate::state::Section;
use crate::ui::widgets::{accent_alpha, sep_color};

use super::widgets;

const NAV: &[(Section, &str)] = &[
    (Section::Inputs, "Inputs"),
    (Section::Sticks, "Sticks"),
    (Section::Triggers, "Triggers"),
    (Section::Haptics, "Haptics"),
    (Section::Gyroscope, "Motion"),
    (Section::Touchpad, "Touchpad"),
    (Section::Lightbar, "Lightbar"),
    (Section::Audio, "Audio"),
    (Section::Profiles, "Profiles"),
    (Section::Advanced, "Advanced"),
    (Section::Settings, "Settings"),
];

impl DS4UApp {
    fn render_nav_item(&mut self, ui: &mut Ui, label: &str, section: Section) {
        let active = self.active_section == section;
        let w = ui.available_width();
        let (rect, resp) = ui.allocate_exact_size(vec2(w, 50.0), Sense::click());
        let c = &self.theme.colors;
        let p = ui.painter();

        if active {
            p.rect_filled(rect, 0.0, widgets::accent_alpha(c, 84));
            p.rect_filled(
                Rect::from_min_size(rect.min, vec2(3.0, rect.height())),
                0.0,
                c.accent(),
            );
        } else if resp.hovered() {
            p.rect_filled(rect, 0.0, widgets::hovered_alpha(c, 32));
        }

        p.rect_filled(
            Rect::from_min_size(pos2(rect.min.x, rect.max.y - 1.0), vec2(rect.width(), 1.0)),
            0.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        );

        let (color, weight) = if active {
            (Color32::WHITE, true)
        } else if resp.hovered() {
            (Color32::from_rgba_unmultiplied(255, 255, 255, 178), false)
        } else {
            (Color32::from_rgba_unmultiplied(255, 255, 255, 102), false)
        };
        let mut rt = RichText::new(label).size(20.0).color(color);
        if weight {
            rt = rt.strong();
        }
        p.text(
            pos2(rect.min.x + 20.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            label,
            egui::FontId::proportional(20.0),
            color,
        );
        let _ = rt;

        if resp.clicked() {
            self.active_section = section;
        }
    }

    pub(crate) fn render_statusbar(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        ui.horizontal_centered(|ui| {
            let prof = self
                .current_profile
                .as_ref()
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "Default".into());
            ui.label(
                RichText::new(format!("DS4U // {}", prof))
                    .size(15.0)
                    .color(Color32::from_rgba_unmultiplied(255, 255, 255, 115)),
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(bat) = self.battery_info.clone() {
                    ui.label(
                        RichText::new(format!("{}%", bat.capacity))
                            .size(15.0)
                            .color(Color32::from_rgba_unmultiplied(255, 255, 255, 153)),
                    );
                    let (r, _) = ui.allocate_exact_size(vec2(32.0, 14.0), Sense::hover());
                    let p = ui.painter();
                    p.rect_stroke(
                        r,
                        CornerRadius::same(2),
                        Stroke::new(1.3, Color32::from_rgba_unmultiplied(255, 255, 255, 102)),
                        StrokeKind::Inside,
                    );
                    let fill_w = (bat.capacity as f32 / 100.0).clamp(0.05, 1.0) * 24.0;
                    p.rect_filled(
                        Rect::from_min_size(r.min + vec2(3.0, 2.5), vec2(fill_w, 9.0)),
                        CornerRadius::same(1),
                        c.accent(),
                    );
                    p.rect_filled(
                        Rect::from_min_size(pos2(r.max.x, r.min.y + 4.0), vec2(3.0, 6.0)),
                        CornerRadius::same(1),
                        Color32::from_rgba_unmultiplied(255, 255, 255, 80),
                    );
                    ui.add_space(26.0);
                }
                let connected = self.is_connected();
                let via_daemon = self.using_daemon();
                ui.label(
                    RichText::new(if connected {
                        "Connected"
                    } else {
                        "Searching..."
                    })
                    .size(15.0)
                    .color(Color32::from_rgba_unmultiplied(255, 255, 255, 153)),
                );
                let (dot, _) = ui.allocate_exact_size(vec2(10.0, 10.0), Sense::hover());
                let col = if connected { c.success() } else { c.warning() };
                ui.painter().circle_filled(dot.center(), 4.0, col);
                if connected && !via_daemon {
                    ui.add_space(8.0);
                    let label = "DIRECT";
                    let pad = vec2(8.0, 3.0);
                    let galley = ui.painter().layout_no_wrap(
                        label.into(),
                        egui::FontId::proportional(11.0),
                        c.warning(),
                    );
                    let size = galley.size() + pad * 2.0;
                    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                    let p = ui.painter();
                    p.rect_stroke(
                        rect,
                        CornerRadius::same(2),
                        Stroke::new(1.0, c.warning()),
                        StrokeKind::Inside,
                    );
                    p.text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        label,
                        egui::FontId::proportional(11.0),
                        c.warning(),
                    );
                }
            });
        });
    }

    pub(crate) fn render_header(&mut self, ui: &mut Ui) {
        let title = match self.active_section {
            Section::Inputs => "Controller Inputs",
            Section::Sticks => "Analog Sticks",
            Section::Triggers => "Adaptive Triggers",
            Section::Haptics => "Haptics & Vibration",
            Section::Gyroscope => "Motion / Gyro",
            Section::Touchpad => "Touchpad",
            Section::Lightbar => "Lightbar & LEDs",
            Section::Audio => "Audio",
            Section::Profiles => "Profiles",
            Section::Advanced => "Advanced",
            Section::Settings => "Settings",
        };
        ui.horizontal_centered(|ui| {
            ui.label(RichText::new(title).size(22.0).strong());
        });
    }

    fn render_title(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), 58.0), Sense::hover());
        let c = &self.theme.colors;
        let p = ui.painter();
        p.rect_filled(
            Rect::from_min_size(pos2(rect.min.x, rect.max.y - 1.0), vec2(rect.width(), 1.0)),
            0.0,
            sep_color(c),
        );
        p.text(
            pos2(rect.min.x + 20.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            "DS4U",
            egui::FontId::proportional(20.0),
            c.accent(),
        );
        let flag = Rect::from_min_size(
            pos2(rect.min.x + 86.0, rect.center().y - 6.0),
            vec2(20.0, 12.0),
        );
        p.rect_filled(
            Rect::from_min_max(flag.min, pos2(flag.max.x, flag.center().y)),
            0.0,
            Color32::from_rgb(0, 87, 183),
        );
        p.rect_filled(
            Rect::from_min_max(pos2(flag.min.x, flag.center().y), flag.max),
            0.0,
            Color32::from_rgb(255, 221, 0),
        );
    }

    pub(crate) fn render_sidebar(&mut self, ui: &mut Ui) {
        self.render_title(ui);

        let connected = self.is_connected();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (sec, label) in NAV {
                    if !connected && *sec != Section::Settings {
                        continue;
                    }
                    self.render_nav_item(ui, label, *sec);
                }

                ui.add_space(32.0);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                    ui.add_space(12.0);

                    if !self.error_message.is_empty() {
                        ui.label(
                            RichText::new(&self.error_message)
                                .size(11.0)
                                .color(self.theme.colors.error()),
                        );
                    }

                    if !self.status_message.is_empty() {
                        ui.label(
                            RichText::new(&self.status_message)
                                .size(11.0)
                                .color(self.theme.colors.success()),
                        );
                    }
                    ui.add_space(8.0);
                });
            });
        let _ = accent_alpha;
    }
}
