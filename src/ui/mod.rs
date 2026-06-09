use eframe::App;
use egui::{
    CentralPanel, Color32, Image, Margin, RichText, SidePanel, TopBottomPanel, Ui,
    containers::Frame, include_image,
};
use std::time::Duration;

use crate::app::DS4UApp;
use crate::inputs::ControllerState;
use crate::state::Section;
use crate::style::apply_style;

pub mod audio;
pub mod firmware;
pub mod gyroscope;
pub mod haptics;
pub mod inputs;
pub mod lightbar;
pub mod profiles;
pub mod settings;
pub mod sidebar;
pub mod sticks;
pub mod touchpad;
pub mod triggers;
pub mod widgets;

impl DS4UApp {
    fn render_main(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| match self.active_section {
            Section::Lightbar => self.render_lightbar_section(ui),
            Section::Triggers => self.render_triggers_section(ui),
            Section::Sticks => self.render_sticks_section(ui),
            Section::Haptics => self.render_haptics_settings(ui),
            Section::Audio => self.render_audio_settings(ui),
            Section::Advanced => self.render_advanced(ui),
            Section::Inputs => self.render_inputs_section(ui),
            Section::Settings => self.render_settings_section(ui),
            Section::Gyroscope => self.render_gyroscope_section(ui),
            Section::Touchpad => self.render_touchpad_section(ui),
            Section::Profiles => self.render_profiles_section(ui),
        });
    }

    fn render_connection(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let time = ui.input(|i| i.time);
            let pulse = (time * 2.0).sin() * 0.3 + 0.7;
            let alpha = (pulse * 255.0) as u8;
            let c = &self.theme.colors;

            let controller_pic = Image::new(include_image!("../../assets/controller.svg"))
                .maintain_aspect_ratio(true)
                .max_width(350.0)
                .tint(Color32::from_white_alpha(alpha));

            ui.add(controller_pic);

            ui.label(
                RichText::new("Connect your DualSense Controller")
                    .size(32.0)
                    .color(c.text()),
            );

            ui.add_space(20.0);

            ui.label(
                RichText::new("Connect via USB cable or Bluetooth")
                    .size(16.0)
                    .color(c.text_dim()),
            );

            ui.add_space(15.0);

            ui.horizontal(|ui| {
                ui.add_space(ui.available_width() / 2.0 - 100.0);

                let spinner = egui::Spinner::new().size(16.0).color(c.accent());

                ui.add(spinner);

                ui.label(
                    RichText::new("Searching for controllers...")
                        .size(14.0)
                        .color(c.accent()),
                );
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
            let needs_input = matches!(
                self.active_section,
                Section::Inputs | Section::Sticks | Section::Touchpad | Section::Gyroscope
            );

            if needs_input {
                if !self.input.polling {
                    self.start_input_polling();
                }

                let states: Vec<ControllerState> = self
                    .input
                    .state_rx
                    .as_ref()
                    .map(|rx| rx.try_iter().collect())
                    .unwrap_or_default();

                if let Some(mut state) = states.into_iter().last() {
                    if !self.using_daemon() {
                        self.input_transform.apply(&mut state);
                    }
                    self.input.controller_state = Some(state);
                }
            } else if self.input.polling {
                self.stop_input_polling();
            }

            if needs_input || self.active_section == Section::Haptics {
                ctx.request_repaint();
            }

            self.check_controller_connection();
            if self.last_battery_update.elapsed() > Duration::from_secs(2) {
                self.update_battery();
            }

            if !needs_input && self.active_section != Section::Haptics {
                ctx.request_repaint_after_secs(2.0);
            }
        }

        self.check_firmware_progress();

        apply_style(ctx, &self.theme);

        let c = self.theme.colors.clone();
        TopBottomPanel::top("statusbar")
            .exact_height(40.0)
            .frame(
                Frame::NONE
                    .fill(c.window_bg())
                    .inner_margin(Margin {
                        left: 32,
                        right: 32,
                        top: 0,
                        bottom: 0,
                    })
                    .stroke(egui::Stroke::new(1.0, crate::ui::widgets::sep_color(&c))),
            )
            .show(ctx, |ui| self.render_statusbar(ui));

        SidePanel::left("ds_sidebar")
            .exact_width(200.0)
            .resizable(false)
            .frame(Frame::NONE.fill(c.panel_bg()))
            .show(ctx, |ui| self.render_sidebar(ui));

        if self.is_connected() {
            TopBottomPanel::top("header")
                .exact_height(58.0)
                .frame(
                    Frame::NONE
                        .fill(c.window_bg())
                        .inner_margin(Margin::symmetric(28, 0))
                        .stroke(egui::Stroke::new(1.0, crate::ui::widgets::sep_color(&c))),
                )
                .show(ctx, |ui| self.render_header(ui));
        }

        CentralPanel::default()
            .frame(Frame::NONE.fill(c.window_bg()))
            .show(ctx, |ui| {
                let bg = ui.available_rect_before_wrap();
                crate::ui::widgets::paint_dotgrid(ui, bg);
                if self.is_connected() || self.active_section == Section::Settings {
                    self.render_main(ui);
                } else {
                    self.render_connection(ui);
                }
            });

        if self.firmware.updating {
            ctx.request_repaint();
        }
    }
}
