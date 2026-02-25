use egui::{Button, Color32, CornerRadius, Frame, Margin, ProgressBar, RichText, Ui, vec2};

use crate::firmware::get_product_name;
use crate::app::DS4UApp;

impl DS4UApp {
    fn render_firmware_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Firmware").size(18.0).strong());

        ui.add_space(14.0);

        let connected = self.is_connected();

        let is_bt = self.controller_is_bt.unwrap_or(false);

        let model = self.controller_product_id
            .map(get_product_name)
            .unwrap_or("-");

        let serial = self.controller_serial.clone().unwrap_or_else(|| "-".to_string());

        let cur_str = self.firmware_current_version
            .map(|v| format!("0x{:04X}", v))
            .unwrap_or_else(|| 
                if connected { "-".into() } else { "Not connected".into() });

        let build_date = self.firmware_build_date.clone().unwrap_or("-".into());
        let build_time = self.firmware_build_time.clone().unwrap_or("-".into());

        let latest_str = self.firmware_latest_version.clone();
        let checking = self.firmware_checking_latest;

        let fw_updating = self.firmware_updating;
        let fw_progress = self.firmware_progress;
        let fw_status   = self.firmware_status.clone();

        let b: Option<bool> = if let (Some(cur), Some(latest)) = 
            (self.firmware_current_version, &latest_str) {
                let latest_int = latest.to_lowercase().trim_start_matches("0x")
                    .parse::<u16>().unwrap();
                Some(latest_int > cur)
            } else {
                None
            };


        Frame::NONE
            .fill(Color32::from_rgb(16, 24, 38))
            .corner_radius(CornerRadius::same(8))
            .inner_margin(Margin::same(14))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                egui::Grid::new("fw_info_grid")
                    .num_columns(2)
                    .spacing([16.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Model").color(Color32::GRAY).size(12.0));
                        ui.label(RichText::new(model).size(12.0));
                        ui.end_row();

                        ui.label(RichText::new("Serial").color(Color32::GRAY).size(12.0));
                        ui.label(RichText::new(serial).size(12.0).monospace());
                        ui.end_row();

                        ui.label(RichText::new("Build Date").color(Color32::GRAY).size(12.0));
                        ui.label(RichText::new(build_date).size(12.0));
                        ui.end_row();

                        ui.label(RichText::new("Build Time").color(Color32::GRAY).size(12.0));
                        ui.label(RichText::new(build_time).size(12.0));
                        ui.end_row();

                        ui.label(RichText::new("Current").color(Color32::GRAY).size(12.0));
                        ui.label(RichText::new(cur_str).size(12.0));
                        ui.end_row();

                        ui.label(RichText::new("Latest").color(Color32::GRAY).size(12.0));
                        ui.horizontal(|ui| {
                            if checking {
                                ui.spinner();
                                ui.label(RichText::new("Checking...").size(12.0));
                            } else if let Some(ref ver) = latest_str {
                                ui.label(RichText::new(ver).size(12.0));
                            } else {
                                ui.label(RichText::new("-").size(12.0));
                                if connected && ui.small_button("Check").clicked() {
                                    self.fetch_latest_verision_async();
                                }
                            }
                        });
                        ui.end_row();
                    });

                if let Some(needs_update) = b {
                    ui.add_space(10.0);
                    if needs_update {
                        ui.colored_label(
                            Color32::from_rgb(255, 190, 50),
                            "Update available"
                        );
                    } else {
                        ui.colored_label(
                            Color32::from_rgb(50, 200, 100),
                            "Firmware is up to date"
                        );
                    }
                }
            });

        ui.add_space(16.0);

        if fw_updating {
            ui.label(RichText::new(&fw_status).color(Color32::GRAY).size(12.0));

            ui.add_space(6.0);

            ui.add(
                ProgressBar::new(fw_progress as f32 / 100.0)
                .text(format!("{}%", fw_progress))
                .animate(true)
            );
        } else if let Some(needs_update) = b && needs_update {
            ui.colored_label(
                Color32::from_rgb(255, 200, 0),
                "USB connection required for flashing"
            );

            ui.add_space(10.0);

            let mut ota_clicked  = false;
            let mut file_clicked = false;

            ui.horizontal(|ui| {
                let ota_btn = Button::new("Download & Update")
                    .min_size(vec2(200.0, 32.0));

                if ui.add_enabled(connected && !is_bt, ota_btn).clicked() {
                    ota_clicked = true;
                }

                ui.add_space(8.0);

                let file_btn = Button::new("Update from File...")
                    .min_size(vec2(160.0, 32.0));

                if ui.add_enabled(connected && !is_bt, file_btn).clicked() {
                    file_clicked = true;
                }
            });

            ui.colored_label(
                Color32::from_rgb(255, 200, 0),
                "WARNING: Do not disconnect controller during update.
Ensure battery is above 10%.
Update can take several minutes.
Controller will disconnect when complete."
            );

            if ota_clicked  { self.flash_latest(); }
            if file_clicked { self.flash_file();   }

            if connected && is_bt {
                ui.add_space(6.0);
                ui.colored_label(
                    Color32::from_rgb(180, 100, 100),
                    "Disconnect Bluetooth and connect via USB to flash"
                );
            }

        }
    }

    pub(crate) fn render_advanced(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Advanced Settings").size(28.0));
        ui.add_space(30.0);

        self.render_firmware_panel(ui);
    }
}
