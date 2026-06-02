use egui::{Button, Color32, ComboBox, Frame, RichText, Stroke, TextEdit, Ui, vec2};

use crate::app::DS4UApp;

impl DS4UApp {
    pub(crate) fn render_profiles_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Profiles").size(28.0));
        ui.add_space(10.0);

        let c = self.theme.colors.clone();
        ui.label(
            RichText::new("Save, switch and manage controller configurations")
                .size(14.0)
                .color(c.text_dim()),
        );

        ui.add_space(20.0);

        let cur_name = self
            .current_profile
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Default".to_string());

        Frame::NONE
            .fill(c.panel_bg())
            .corner_radius(8)
            .inner_margin(14)
            .show(ui, |ui| {
                ui.label(
                    RichText::new("Active profile")
                        .size(12.0)
                        .color(c.text_dim()),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(&cur_name).size(20.0).color(c.text()).strong());

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui
                        .add(Button::new("Save current state").min_size(vec2(160.0, 30.0)))
                        .clicked()
                    {
                        self.sync_profile();
                        self.status_message = format!("Saved profile '{}'", cur_name);
                    }

                    if ui
                        .add(Button::new("Reload from disk").min_size(vec2(140.0, 30.0)))
                        .clicked()
                        && let Ok(p) = self.profile_manager.load_profile(&cur_name)
                    {
                        self.load_profile(&p);
                        self.status_message = format!("Reloaded '{}'", cur_name);
                    }
                });
            });

        ui.add_space(20.0);

        ui.label(RichText::new("Create new profile").size(16.0).strong());
        ui.add_space(8.0);

        let mut do_create = false;

        ui.horizontal(|ui| {
            let edit = TextEdit::singleline(&mut self.profile_edit_name)
                .hint_text("Profile name")
                .desired_width(200.0);
            ui.add(edit);

            let enabled = !self.profile_edit_name.trim().is_empty()
                && !self
                    .profile_manager
                    .profile_exists(self.profile_edit_name.trim());

            if ui
                .add_enabled(enabled, Button::new("Create").min_size(vec2(100.0, 28.0)))
                .clicked()
            {
                do_create = true;
            }
        });

        if do_create {
            let name = self.profile_edit_name.trim().to_string();
            if self.create_profile(&name) {
                self.status_message = format!("Created profile '{}'", name);
                self.profile_edit_name.clear();
            } else {
                self.error_message = "Failed to create profile".to_string();
            }
        }

        ui.add_space(20.0);
        ui.separator();
        ui.add_space(20.0);

        ui.label(RichText::new("All profiles").size(16.0).strong());
        ui.add_space(10.0);

        let profiles = self.profile_manager.list_profiles();
        if profiles.is_empty() {
            ui.label(RichText::new("(no profiles found)").color(c.text_dim()));
            return;
        }

        let mut switch_to: Option<String> = None;
        let mut delete: Option<String> = None;

        for p in &profiles {
            let selected = p.name == cur_name;
            let row_frame_color = if selected {
                c.accent()
            } else {
                Color32::TRANSPARENT
            };

            Frame::NONE
                .fill(c.panel_bg())
                .stroke(Stroke::new(
                    if selected { 2.0 } else { 1.0 },
                    row_frame_color,
                ))
                .corner_radius(6)
                .inner_margin(10)
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&p.name).size(14.0).color(if selected {
                            c.accent()
                        } else {
                            c.text()
                        }));

                        ui.add_space(ui.available_width() - 200.0);

                        if !selected
                            && ui
                                .add(Button::new("Activate").min_size(vec2(80.0, 24.0)))
                                .clicked()
                        {
                            switch_to = Some(p.name.clone());
                        }

                        if p.name != "Default"
                            && ui
                                .add(
                                    Button::new("Delete")
                                        .fill(c.error())
                                        .min_size(vec2(70.0, 24.0)),
                                )
                                .clicked()
                        {
                            delete = Some(p.name.clone());
                        }
                    });
                });

            ui.add_space(4.0);
        }

        if let Some(name) = switch_to
            && let Ok(p) = self.profile_manager.load_profile(&name)
        {
            self.load_profile(&p);
            self.status_message = format!("Activated '{}'", name);
        }

        if let Some(name) = delete {
            if self.delete_profile(&name) {
                self.status_message = format!("Deleted '{}'", name);
            } else {
                self.error_message = format!("Failed to delete '{}'", name);
            }
        }
    }
}
