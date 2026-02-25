use std::time::Duration;
use eframe::App;
use egui::{CentralPanel, Color32, Image, RichText, SidePanel, Ui, include_image};

use crate::app::{DS4UApp};
use crate::state::Section;
use crate::style::apply_style;

pub mod audio;
pub mod firmware;
pub mod haptics;
pub mod inputs;
pub mod lightbar;
pub mod sidebar;
pub mod sticks;
pub mod triggers;
pub mod settings;

impl DS4UApp {
    fn render_main(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(30.0);

            match self.active_section {
                Section::Lightbar   => self.render_lightbar_section(ui),
                Section::Triggers   => self.render_triggers_section(ui),
                Section::Sticks     => self.render_sticks_section(ui),
                Section::Haptics    => self.render_haptics_settings(ui),
                Section::Audio      => self.render_audio_settings(ui),
                Section::Advanced   => self.render_advanced(ui),
                Section::Inputs     => self.render_inputs_section(ui),
                Section::Settings   => self.render_settings_section(ui)
            }

            ui.add_space(30.0);
        });
    }

    fn render_connection(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let time = ui.input(|i| i.time);
            let pulse = (time * 2.0).sin() * 0.3 + 0.7;
            let alpha = (pulse * 255.0) as u8;

            let controller_pic = Image::new(include_image!("../../assets/controller.svg"))
                .maintain_aspect_ratio(true)
                .max_width(350.0)
                .tint(Color32::from_white_alpha(alpha));

            ui.add(controller_pic);

            ui.label(RichText::new("Connect your DualSense Controller")
                .size(32.0)
                .color(Color32::WHITE));

            ui.add_space(20.0);

            ui.label(RichText::new("Connect via USB cable or Bluetooth")
                .size(16.0)
                .color(Color32::GRAY));

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 100.0);

                let spinner = egui::Spinner::new()
                    .size(16.0)
                    .color(Color32::from_rgb(0, 112, 220));

                ui.add(spinner);

                ui.label(RichText::new("Searching for controllers...")
                    .size(14.0)
                    .color(Color32::from_rgb(0, 112, 220)));
                    });
        });
    }
}

impl App for DS4UApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.is_connected() {
            if self.last_connection_check.elapsed() > Duration::from_millis(200) {
                self.check_for_controller();
            }
            ctx.request_repaint_after_secs(0.2);
        } else {
            if self.active_section == Section::Inputs {
                if !self.input_polling {
                    self.start_input_polling();
                }

                if let Some(rx) = &self.input_state_rx {
                    while let Ok(mut state) = rx.try_recv() {
                        if self.ipc.is_none() {
                            self.input_transform.apply(&mut state);
                        }
                        self.controller_state = Some(state);
                    }
                }

                ctx.request_repaint();
            } else if self.input_polling {
                self.stop_input_polling();
            }

            self.check_controller_connection();
            if self.last_battery_update.elapsed() > Duration::from_secs(2) {
                self.update_battery();
            }
            ctx.request_repaint_after_secs(2.0);
        }

        self.check_firmware_progress();

        apply_style(ctx, &self.theme);

        SidePanel::left("sidebar")
            .exact_width(280.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            if self.is_connected() {
                self.render_main(ui);
            } else {
                self.render_connection(ui);
            }
        });

        if self.firmware_updating {
            ctx.request_repaint();
        }
    }
}

