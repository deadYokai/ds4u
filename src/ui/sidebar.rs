use egui::{Button, Color32, CornerRadius, Frame, Layout, Margin, RichText, Sense, Ui, vec2};

use crate::app::DS4UApp;
use crate::state::Section;

impl DS4UApp {
    fn render_nav_btn(&mut self, ui: &mut Ui, label: &str, section: Section) {
        let is_active = self.active_section == section;

        let btn = Button::new(RichText::new(label).size(14.0))
            .fill(if is_active {
                Color32::from_rgb(0, 112, 220)
            } else {
                Color32::TRANSPARENT
            })
        .stroke(egui::Stroke::NONE)
            .min_size(vec2(ui.available_width(), 40.0));

        if ui.add(btn).clicked() {
            self.active_section = section;
        }
    }

    fn render_connection_status(&mut self, ui: &mut Ui) {
        Frame::NONE
            .fill(Color32::from_rgb(20, 30, 50))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                if self.is_connected() {
                    let daemon_color = if self.ipc.is_some() {
                        Color32::GREEN
                    } else { Color32::WHITE };
                    if let Some(battery) = &self.battery_info {
                        ui.label(RichText::new(
                                format!("Connected • {}", battery.status)
                        )
                            .size(12.0)
                            .color(daemon_color));
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.label(format!("{}%", battery.capacity));
                                let battery_color = if battery.capacity > 50 {
                                    Color32::from_rgb(0, 200, 100)
                                } else if battery.capacity > 20 {
                                    Color32::from_rgb(255, 180, 0)
                                } else {
                                    Color32::from_rgb(255, 50, 50)
                                };

                                let bar_width = ui.available_width();
                                let (rect, _) = ui.allocate_exact_size(
                                    vec2(bar_width, 4.0),
                                    egui::Sense::hover()
                                );

                                ui.painter().rect_filled(
                                    egui::Rect::from_min_size(
                                        rect.min,
                                        vec2(bar_width * 
                                            (battery.capacity as f32 / 100.0), 4.0)
                                    ),
                                    2.0,
                                    battery_color
                                ); 
                            });
                    } else { 
                        ui.label(RichText::new("Connected")
                            .size(12.0)
                            .color(daemon_color));
                            }
                } else { 
                    let spinner = egui::Spinner::new()
                        .size(12.0)
                        .color(Color32::from_rgb(0, 112, 220));

                    ui.add(spinner);

                    ui.label(RichText::new("Searching...")
                        .size(12.0)
                        .color(Color32::from_rgb(0, 112, 220)));
                        }
            });
    }

    pub(crate) fn render_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);

        ui.with_layout(Layout::top_down(egui::Align::Min), |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("DS4U").size(24.0)
                    .color(Color32::WHITE).strong());

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
        ui.separator();
        ui.add_space(20.0);

        if self.is_connected() {
            // ui.label(RichText::new("Profile")
            //     .size(12.0)
            //     .color(Color32::GRAY));
            //
            // ui.add_space(5.0);
            //
            // egui::ComboBox::from_id_salt("profile_combo")
            //     .selected_text(self.current_profile.as_ref()
            //         .map(|p| p.name.as_str())
            //         .unwrap_or("Default"))
            //     .width(ui.available_width())
            //     .show_ui(ui, |ui| {
            //         if ui.selectable_label
            //             (self.current_profile.is_none(), "Default").clicked() {
            //             self.current_profile = None;
            //         }
            //
            //         for profile in self.profile_manager.list_profiles() {
            //             if ui.selectable_label(
            //                     self.current_profile.as_ref()
            //                         .map(|p| &p.name) == Some(&profile.name),
            //                     &profile.name)
            //                 .clicked() {
            //                     self.load_profile(&profile);
            //             }
            //         }
            //     });
            //
            // ui.add_space(10.0);
            //
            // if ui.button("Manage Profiles").clicked() {
            //     self.show_profiles_panel = !self.show_profiles_panel;
            // }
            //
            // ui.add_space(30.0);
            // ui.separator();
            // ui.add_space(20.0);

            self.render_nav_btn(ui, "Inputs", Section::Inputs);
            self.render_nav_btn(ui, "Lightbar", Section::Lightbar);
            self.render_nav_btn(ui, "Triggers", Section::Triggers);
            self.render_nav_btn(ui, "Sticks", Section::Sticks);
            self.render_nav_btn(ui, "Haptics", Section::Haptics);
            self.render_nav_btn(ui, "Audio", Section::Audio);
            self.render_nav_btn(ui, "Advanced", Section::Advanced);
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);

            if !self.error_message.is_empty() {
                ui.label(RichText::new(&self.error_message)
                    .size(11.0)
                    .color(Color32::from_rgb(255, 100, 100)));
            }

            if !self.status_message.is_empty() {
                ui.label(RichText::new(&self.status_message)
                    .size(11.0)
                    .color(Color32::from_rgb(100, 255, 100)));
            }
        });
    }

}
