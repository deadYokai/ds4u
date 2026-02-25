use egui::{Button, Color32, RichText, Slider, Ui, vec2};

use crate::app::DS4UApp;

impl DS4UApp {
    pub(crate) fn render_lightbar_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Lightbar & Indicators").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Customize your controller lights")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Color").size(16.0).strong());

            ui.add_space(20.0);

            let mut color = [self.lightbar.r, self.lightbar.g, self.lightbar.b];

            if ui.color_edit_button_rgb(&mut color).changed() {
                self.lightbar.r = color[0];
                self.lightbar.g = color[1];
                self.lightbar.b = color[2];
                self.apply_lightbar();
            }

            ui.add_space(20.0);

            ui.label("Presets:");

            for (name, r, g, b) in [
                ("Aesthetic", 0.0, 0.5, 1.0),
                ("DevColor", 1.0, 0.63, 0.0),
                ("Blue", 0.0, 0.0, 1.0),
                ("Red", 1.0, 0.0, 0.0),
                ("Green", 0.0, 1.0, 0.0),
                ("Purple", 0.8, 0.0, 1.0),
                ("White", 1.0, 1.0, 1.0)
            ] {
                let color_btn = Button::new(" ")
                    .fill(Color32::from_rgb(
                            (r * 255.0) as u8,
                            (g * 255.0) as u8,
                            (b * 255.0) as u8
                    ))
                    .min_size(vec2(32.0, 32.0));

                if ui.add(color_btn).on_hover_text(name).clicked() {
                    self.lightbar.r = r;
                    self.lightbar.g = g;
                    self.lightbar.b = b;
                    self.apply_lightbar();
                }
            }
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            ui.label(RichText::new("Brightness").size(16.0).strong());

            ui.add_space(20.0);

            if ui.add(Slider::new(&mut self.lightbar.brightness, 0.0..=255.0)
                .text("").show_value(false)).changed() {
                self.apply_lightbar();
            }

            ui.label(format!("{}%", (self.lightbar.brightness / 255.0 * 100.0) as u8));
        });
        ui.add_space(30.0);
        ui.separator();
        ui.add_space(30.0);

        ui.label(RichText::new("Player Indicator").size(16.0).strong());

        ui.add_space(15.0);

        ui.horizontal(|ui| {
            for i in 0..=7 {
                let btn = Button::new(format!("{}", i + 1))
                    .fill(if self.player_leds == i {
                        Color32::from_rgb(0, 112, 220)
                    } else {
                        Color32::from_rgb(30, 40, 60)
                    }).min_size(vec2(48.0, 48.0));

                if ui.add(btn).clicked() {
                    self.player_leds = i;
                    self.apply_player_leds();
                }
            }
        });
    }

}
