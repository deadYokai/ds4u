use egui::{Color32, RichText, Slider, Ui};

use crate::app::DS4UApp;
use crate::common::{MicLedState, SpeakerMode};

impl DS4UApp { 
    pub(crate) fn render_audio_settings(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Microphone & Audio").size(28.0));
        ui.add_space(10.0);

        ui.label(RichText::new("Configure Audio")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.label(RichText::new("Microphone").size(18.0).strong());
        ui.add_space(10.0);

        if ui.checkbox(&mut self.microphone.enabled, "Microphone Enabled").changed() {
            self.apply_microphone();
        }

        ui.add_space(20.0);

        ui.label("Mic LED:");
        ui.horizontal(|ui| {
            if ui.selectable_value(&mut self.microphone.led_state, MicLedState::Off, "Off")
                .clicked() {
                    self.apply_microphone();
            }
            if ui.selectable_value(&mut self.microphone.led_state, MicLedState::On, "On")
                .clicked() {
                    self.apply_microphone();
            }
            if ui.selectable_value(&mut self.microphone.led_state, MicLedState::Pulse, "Pulse")
                .clicked() {
                    self.apply_microphone();
            }
        });

        ui.add_space(30.0);
        ui.separator();
        ui.add_space(30.0);

        ui.label(RichText::new("Speaker Mode").size(18.0).strong());
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            if ui.selectable_label(
                self.audio.speaker_mode == SpeakerMode::Internal,
                "Internal Speaker"
            ).clicked() {
                self.audio.speaker_mode = SpeakerMode::Internal;
                let mode_str = "internal";
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap().set_speaker(mode_str);
                } else if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                        let _ = ctrl.set_speaker(mode_str);
                }
            }

            if ui.selectable_label(
                self.audio.speaker_mode == SpeakerMode::Headphone,
                "Headphone"
            ).clicked() {
                self.audio.speaker_mode = SpeakerMode::Headphone;
                let mode_str = "headphone";
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap().set_speaker(mode_str);
                } else if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                        let _ = ctrl.set_speaker(mode_str);
                }
            }

            if ui.selectable_label(
                self.audio.speaker_mode == SpeakerMode::Both,
                "Both"
            ).clicked() {
                self.audio.speaker_mode = SpeakerMode::Both;
                let mode_str = "both";
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap().set_speaker(mode_str);
                } else if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                        let _ = ctrl.set_speaker(mode_str);
                }
            }

        });

        ui.add_space(30.0);
        ui.separator();
        ui.add_space(30.0);

        ui.label(RichText::new("Volume").size(18.0).strong());
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Level:");
            if ui.add(Slider::new(&mut self.audio.volume, 0..=255).text(""))
                .changed()
            {
                let vol = self.audio.volume;
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap().set_volume(vol);
                } else if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock()
                {
                        let _ = ctrl.set_volume(vol); 
                }
            }
        });
    }

}
