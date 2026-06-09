use egui::{Color32, RichText, TextEdit, Ui, vec2};

use crate::app::DS4UApp;

use super::widgets::{ds_label, ds_pill_button, ds_row, ds_section, ds_value_text};

impl DS4UApp {
    pub(crate) fn render_profiles_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();

        let cur_name = self
            .current_profile
            .as_ref()
            .map(|p| p.name.clone())
            .unwrap_or_else(|| "Default".to_string());

        let mut do_create = false;
        let mut switch_to: Option<String> = None;
        let mut delete: Option<String> = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ds_section(ui, &c, "Active");
                ds_row(ui, |ui| {
                    ds_label(ui, &cur_name);
                    if ds_pill_button(ui, &c, "Save", false).clicked() {
                        self.sync_profile();
                        self.status_message = format!("Saved profile '{}'", cur_name);
                    }
                    ui.add_space(8.0);
                    if ds_pill_button(ui, &c, "Reload", false).clicked()
                        && let Ok(p) = self.profile_manager.load_profile(&cur_name)
                    {
                        self.load_profile(&p);
                        self.status_message = format!("Reloaded '{}'", cur_name);
                    }
                });

                ds_section(ui, &c, "Create");
                ds_row(ui, |ui| {
                    ds_label(ui, "Name");
                    ui.add(
                        TextEdit::singleline(&mut self.profile_edit_name)
                            .hint_text("Profile name")
                            .desired_width(220.0),
                    );
                    ui.add_space(8.0);
                    let enabled = !self.profile_edit_name.trim().is_empty()
                        && !self
                            .profile_manager
                            .profile_exists(self.profile_edit_name.trim());
                    let resp = ds_pill_button(ui, &c, "Create", false);
                    if enabled && resp.clicked() {
                        do_create = true;
                    }
                });

                ds_section(ui, &c, "All Profiles");
                for p in self.profile_manager.list_profiles() {
                    let selected = p.name == cur_name;
                    let name = p.name.clone();
                    ds_row(ui, |ui| {
                        let lbl = if selected {
                            RichText::new(&name).size(20.0).color(c.accent()).strong()
                        } else {
                            RichText::new(&name)
                                .size(20.0)
                                .color(Color32::from_rgba_unmultiplied(255, 255, 255, 204))
                        };
                        ui.add_sized(
                            vec2(
                                crate::ui::widgets::LBL_WIDTH,
                                crate::ui::widgets::ROW_HEIGHT,
                            ),
                            egui::Label::new(lbl).selectable(false),
                        );

                        if !selected && ds_pill_button(ui, &c, "Activate", false).clicked() {
                            switch_to = Some(name.clone());
                        }
                        if name != "Default" {
                            ui.add_space(8.0);
                            let resp = ds_pill_button(ui, &c, "Delete", false);
                            if resp.clicked() {
                                delete = Some(name.clone());
                            }
                        }
                        let tag = if selected { "Active" } else { "Saved" };
                        ds_value_text(ui, tag);
                    });
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
