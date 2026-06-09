use egui::{Color32, RichText, Sense, Stroke, StrokeKind, Ui, vec2};

use crate::app::DS4UApp;
use crate::ui::widgets::{ROW_PAD_X, ds_label, ds_pill_button, ds_row, ds_section};

impl DS4UApp {
    pub(crate) fn render_settings_section(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ds_section(ui, &c, "Theme");
                ds_row(ui, |ui| {
                    ds_label(ui, "Active");
                    ui.label(
                        RichText::new(&self.theme.name)
                            .size(20.0)
                            .strong()
                            .color(c.accent()),
                    );
                    ui.add_space(12.0);
                    if ds_pill_button(ui, &c, "Refresh", false).clicked() {
                        self.theme_manager.reload();
                    }
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.add_space(ROW_PAD_X);
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                        ui.spacing_mut().item_spacing = vec2(12.0, 12.0);
                        let themes = self.theme_manager.list_all().to_vec();
                        for t in themes {
                            let selected = t.id == self.theme.id;
                            let tc = t.colors.clone();

                            let (rect, resp) =
                                ui.allocate_exact_size(vec2(180.0, 90.0), Sense::click());
                            let p = ui.painter();
                            p.rect_filled(
                                rect,
                                4.0,
                                Color32::from_rgb(tc.panel_bg[0], tc.panel_bg[1], tc.panel_bg[2]),
                            );
                            let stroke_col = if selected {
                                c.accent()
                            } else if resp.hovered() {
                                crate::ui::widgets::accent_alpha(&c, 180)
                            } else {
                                crate::ui::widgets::sep_color(&c)
                            };
                            p.rect_stroke(
                                rect,
                                4.0,
                                Stroke::new(if selected { 2.0 } else { 1.0 }, stroke_col),
                                StrokeKind::Inside,
                            );
                            for (i, col) in [tc.accent, tc.success, tc.error].iter().enumerate() {
                                let s = egui::Rect::from_min_size(
                                    egui::pos2(
                                        rect.min.x + 12.0 + i as f32 * 22.0,
                                        rect.min.y + 12.0,
                                    ),
                                    vec2(18.0, 18.0),
                                );
                                p.rect_filled(s, 3.0, Color32::from_rgb(col[0], col[1], col[2]));
                            }
                            p.text(
                                egui::pos2(rect.min.x + 12.0, rect.max.y - 14.0),
                                egui::Align2::LEFT_BOTTOM,
                                &t.name,
                                egui::FontId::proportional(16.0),
                                Color32::from_rgb(tc.text[0], tc.text[1], tc.text[2]),
                            );

                            if resp.clicked() && !selected {
                                self.settings.theme_id = t.id.clone();
                                self.theme = t.clone();
                                self.settings_manager.save(&self.settings);
                            }
                        }
                    });
                });

                ds_section(ui, &c, "General");
                ds_row(ui, |ui| {
                    ds_label(ui, "TODO:");
                    ui.label(
                        RichText::new("Daemon settings")
                            .size(18.0)
                            .italics()
                            .color(c.text_dim()),
                    );
                });
            });
    }
}
