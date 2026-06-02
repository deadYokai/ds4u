use egui::{
    Button, Color32, ComboBox, CornerRadius, Frame, Layout, Margin, RichText, Sense, Ui, vec2,
};

use crate::app::DS4UApp;
use crate::state::Section;

impl DS4UApp {
    fn render_nav_btn(&mut self, ui: &mut Ui, label: &str, section: Section) {
        let is_active = self.active_section == section;

        let btn = Button::new(RichText::new(label).size(14.0))
            .fill(if is_active {
                self.theme.colors.accent()
            } else {
                Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::NONE)
            .min_size(vec2(ui.available_width(), 40.0));

        if ui.add(btn).clicked() {
            self.active_section = section;
        }
    }

    fn render_profile_selector(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        let cur_name = self
            .current_profile
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Default".to_string());

        ui.label(RichText::new("Profile").size(12.0).color(c.text_dim()));
        ui.add_space(4.0);

        let mut switch_to: Option<String> = None;

        ComboBox::from_id_salt("profile_combo")
            .selected_text(cur_name.clone())
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                let profiles = self.profile_manager.list_profiles();
                if profiles.is_empty() {
                    ui.label(RichText::new("(no profiles)").color(c.text_dim()));
                } else {
                    for p in profiles {
                        if ui.selectable_label(p.name == cur_name, &p.name).clicked() {
                            switch_to = Some(p.name);
                        }
                    }
                }
            });

        if let Some(name) = switch_to
            && let Ok(p) = self.profile_manager.load_profile(&name)
        {
            self.load_profile(&p);
        }

        ui.add_space(6.0);
        if ui
            .button(RichText::new("Manage profiles").size(12.0))
            .clicked()
        {
            self.active_section = Section::Profiles;
        }
    }

    fn render_connection_status(&mut self, ui: &mut Ui) {
        let c = &self.theme.colors;
        Frame::NONE
            .fill(c.panel_bg())
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                if self.is_connected() {
                    let daemon_color = if self.ipc.is_some() {
                        c.success()
                    } else {
                        c.text()
                    };
                    if let Some(battery) = &self.battery_info {
                        ui.label(
                            RichText::new(format!("Connected • {}", battery.status))
                                .size(12.0)
                                .color(daemon_color),
                        );
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(format!("{}%", battery.capacity));
                            let battery_color = if battery.capacity > 50 {
                                c.success()
                            } else if battery.capacity > 20 {
                                c.warning()
                            } else {
                                c.error()
                            };

                            let bar_width = ui.available_width();
                            let (rect, _) =
                                ui.allocate_exact_size(vec2(bar_width, 4.0), egui::Sense::hover());

                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min,
                                    vec2(bar_width * (battery.capacity as f32 / 100.0), 4.0),
                                ),
                                2.0,
                                battery_color,
                            );
                        });
                    } else {
                        ui.label(RichText::new("Connected").size(12.0).color(daemon_color));
                    }
                } else {
                    let spinner = egui::Spinner::new().size(12.0).color(c.accent());

                    ui.add(spinner);

                    ui.label(RichText::new("Searching...").size(12.0).color(c.accent()));
                }
            });
    }

    pub(crate) fn render_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);

        ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new("DS4U")
                        .size(24.0)
                        .color(self.theme.colors.text())
                        .strong(),
                );

                let (rect, _) = ui.allocate_exact_size(vec2(32.0, 18.0), Sense::hover());
                let p = ui.painter();
                let top = egui::Rect::from_min_max(rect.min, rect.min + vec2(32.0, 9.0));
                let bot = egui::Rect::from_min_max(rect.min + vec2(0.0, 9.0), rect.max);

                p.rect_filled(top, 0.0, Color32::from_rgb(0, 87, 183));
                p.rect_filled(bot, 0.0, Color32::from_rgb(255, 221, 0));
            });

            self.render_connection_status(ui);
        });

        ui.add_space(5.0);

        if self.is_connected() {
            self.render_profile_selector(ui);
        }

        ui.add_space(5.0);
        ui.separator();
        ui.add_space(20.0);

        if self.is_connected() {
            self.render_nav_btn(ui, "Inputs", Section::Inputs);
            self.render_nav_btn(ui, "Lightbar", Section::Lightbar);
            self.render_nav_btn(ui, "Sticks", Section::Sticks);
            self.render_nav_btn(ui, "Triggers", Section::Triggers);
            self.render_nav_btn(ui, "Haptics", Section::Haptics);
            self.render_nav_btn(ui, "Gyroscope", Section::Gyroscope);
            self.render_nav_btn(ui, "Touchpad", Section::Touchpad);
            self.render_nav_btn(ui, "Audio", Section::Audio);
            self.render_nav_btn(ui, "Profiles", Section::Profiles);
            self.render_nav_btn(ui, "Advanced", Section::Advanced);
            self.render_nav_btn(ui, "Settings", Section::Settings);
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);

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
        });
    }
}
