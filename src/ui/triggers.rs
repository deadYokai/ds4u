use egui::{Color32, RichText, Slider, Ui};

use crate::app::DS4UApp;
use crate::common::TriggerMode;

impl DS4UApp {
    pub(crate) fn render_triggers_section(&mut self, ui: &mut Ui) { 
        ui.heading(RichText::new("Adaptive Triggers").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Configure trigger resistance and feedback")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.label(RichText::new("Effect mode").size(16.0).strong());

        ui.add_space(15.0);

        ui.horizontal(|ui| {
            if ui.selectable_label
                (self.triggers.mode == TriggerMode::Off, "Off").clicked() {
                    self.triggers.mode = TriggerMode::Off;
                    self.apply_trigger();
            }

            if ui.selectable_label
                (self.triggers.mode == TriggerMode::Feedback, "Feedback").clicked() {
                    self.triggers.mode = TriggerMode::Feedback;
            }
        });

        if self.triggers.mode == TriggerMode::Feedback {
            ui.add_space(30.0);

            ui.label(RichText::new("Position").size(14.0));
            ui.add(Slider::new(&mut self.triggers.position, 0..=9));

            ui.add_space(15.0);

            ui.label(RichText::new("Strength").size(14.0));
            ui.add(Slider::new(&mut self.triggers.strength, 1..=8));

            ui.add_space(20.0);

            if ui.button("Apply").clicked() {
                self.apply_trigger();
            }
        }
    }
}
