use egui::{Color32, RichText, Slider, Ui};

use crate::app::DS4UApp;

impl DS4UApp {
    pub(crate) fn render_haptics_settings(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Haptic Settings").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Configure vibration and haptic feedback")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.label(RichText::new("Vibration Attenuation").size(18.0).strong());

        ui.add_space(6.0);

        ui.label(
            RichText::new("0 = full strength · 7 = quietest")
            .size(12.0)
            .color(Color32::GRAY),
        );

        ui.add_space(10.0);

        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("Rumble Motors:");

            if ui.add(Slider::new(&mut self.vibration.rumble, 0..=7).text(""))
                .changed() 
            {
                changed = true;
            }

            ui.label(format!("{}", self.vibration.rumble));
        });

        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Trigger Vibration:");

            if ui.add(Slider::new(&mut self.vibration.trigger, 0..=7).text(""))
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
