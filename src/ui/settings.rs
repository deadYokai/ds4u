use egui::{vec2, Color32, Frame, RichText, Stroke, Ui};

use crate::app::DS4UApp;

impl DS4UApp {
    pub(crate) fn render_settings_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Settings").size(28.0));
        ui.add_space(10.0);

        ui.label(RichText::new("Application preferences")
            .size(14.0).color(self.theme.colors.text_dim()));

        ui.add_space(30.0);

        ui.label(RichText::new("Theme").size(18.0).strong());
        ui.add_space(12.0);

        let themes = self.theme_manager.list_all();

        egui::Grid::new("theme_grid")
            .num_columns(3)
            .spacing(vec2(12.0, 12.0))
            .show(ui, |ui| {
                for (i, t) in themes.iter().enumerate() {
                    let selected = t.id == self.theme.id;
                    let c = &t.colors;

                    let frame_color = if selected {
                        self.theme.colors.accent()
                    } else {
                        Color32::TRANSPARENT
                    };

                    let response = Frame::NONE
                        .fill(Color32::from_rgb(c.panel_bg[0], c.panel_bg[1], c.panel_bg[2]))
                        .stroke(Stroke::new(if selected { 2.0 } else { 1.0 }, frame_color))
                        .corner_radius(8)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.set_min_width(180.0);

                            ui.horizontal(|ui| {
                                for col in [c.accent, c.success, c.error] {
                                    let (rect, _) = ui.allocate_exact_size(
                                        vec2(16.0, 16.0),
                                        egui::Sense::hover()
                                    );

                                    ui.painter().rect_filled(
                                        rect, 4.0,
                                        Color32::from_rgb(col[0], col[1], col[2])
                                    );
                                }
                            });

                            ui.add_space(6.0);

                            ui.label(RichText::new(&t.name).size(13.0)
                                .color(Color32::from_rgb(c.text[0], c.text[1], c.text[2])));
                    }).response;
                    
                    if response.interact(egui::Sense::click()).clicked() && !selected {
                        self.settings.theme_id = t.id.clone();
                        self.theme = t.clone();
                        self.settings_manager.save(&self.settings);
                    }

                    if (i + 1) % 3 == 0 {
                        ui.end_row();
                    }
                }
        });

        ui.add_space(30.0);
        ui.separator();
        ui.add_space(30.0);
        
        ui.label(RichText::new("General").size(18.0).strong());
        ui.add_space(10.0);
        ui.label(RichText::new("Nothing here yet...")
            .size(14.0).color(self.theme.colors.text_dim()));
    }
}
