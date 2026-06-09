use egui::Ui;

use crate::app::DS4UApp;
use crate::common::{MicLedState, SpeakerMode};
use crate::ui::widgets::{
    ds_label, ds_pill_button, ds_row, ds_section, ds_slider_int, ds_toggle, ds_value_pct,
};

impl DS4UApp {
    pub(crate) fn render_audio_settings(&mut self, ui: &mut Ui) {
        let c = self.theme.colors.clone();

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ds_section(ui, &c, "Microphone");
                ds_row(ui, |ui| {
                    ds_label(ui, "Enabled");
                    if ds_toggle(ui, &c, &mut self.microphone.enabled).changed() {
                        self.apply_microphone();
                        self.sync_profile();
                    }
                });
                ds_row(ui, |ui| {
                    ds_label(ui, "Mic LED");
                    ui.horizontal_wrapped(|ui| {
                        for (state, label) in [
                            (MicLedState::Off, "Off"),
                            (MicLedState::On, "On"),
                            (MicLedState::Pulse, "Pulse"),
                        ] {
                            let active = self.microphone.led_state == state;
                            if ds_pill_button(ui, &c, label, active).clicked() && !active {
                                self.microphone.led_state = state;
                                self.apply_microphone();
                            }
                        }
                    });
                });

                ds_section(ui, &c, "Speaker");
                ds_row(ui, |ui| {
                    ds_label(ui, "Output");
                    let mut switch = |ui: &mut Ui, mode: SpeakerMode, label: &str, key: &str| {
                        let active = self.audio.speaker_mode == mode;
                        if ds_pill_button(ui, &c, label, active).clicked() && !active {
                            self.audio.speaker_mode = mode;
                            if let Some(ipc) = self.ipc.clone() {
                                let _ = ipc.lock().unwrap().set_speaker(key);
                            } else if let Some(controller) = &self.controller {
                                if let Ok(mut ctrl) = controller.lock() {
                                    let _ = ctrl.set_speaker(key);
                                }
                            }
                        }
                    };
                    ui.horizontal_wrapped(|ui| {
                        switch(ui, SpeakerMode::Internal, "Internal", "internal");
                        switch(ui, SpeakerMode::Headphone, "Headphone", "headphone");
                        switch(ui, SpeakerMode::Both, "Both", "both");
                    });
                });

                ds_section(ui, &c, "Volume");
                ds_row(ui, |ui| {
                    ds_label(ui, "Level");
                    let mut vol = self.audio.volume as i32;
                    if ds_slider_int(ui, &c, &mut vol, 0..=255).changed() {
                        self.audio.volume = vol as u8;
                        if let Some(ipc) = self.ipc.clone() {
                            let _ = ipc.lock().unwrap().set_volume(vol as u8);
                        } else if let Some(controller) = &self.controller {
                            if let Ok(mut ctrl) = controller.lock() {
                                let _ = ctrl.set_volume(vol as u8);
                            }
                        }
                    }
                    ds_value_pct(ui, (self.audio.volume as f32 / 255.0) * 100.0);
                });
            });
    }
}
