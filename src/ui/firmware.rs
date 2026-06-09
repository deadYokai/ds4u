use egui::{ProgressBar, RichText, Ui};

use crate::app::DS4UApp;
use crate::firmware::get_product_name;
use crate::ui::widgets::{
    ROW_PAD_X, ds_label, ds_pill_button, ds_row, ds_section, ds_value_text, ds_value_text_lr,
};

impl DS4UApp {
    fn render_firmware_panel(&mut self, ui: &mut Ui) {
        let connected = self.is_connected();

        let is_bt = self.controller_is_bt.unwrap_or(false);

        let model = self
            .controller_product_id
            .map(get_product_name)
            .unwrap_or("-");

        let serial = self
            .controller_serial
            .clone()
            .unwrap_or_else(|| "-".to_string());

        let cur_str = self
            .firmware
            .current_version
            .map(|v| format!("0x{:04X}", v))
            .unwrap_or_else(|| {
                if connected {
                    "-".into()
                } else {
                    "Not connected".into()
                }
            });

        let build_date = self.firmware.build_date.clone().unwrap_or("-".into());
        let build_time = self.firmware.build_time.clone().unwrap_or("-".into());

        let latest_str = self.firmware.latest_version.clone();
        let checking = self.firmware.checking_latest;

        let fw_updating = self.firmware.updating;
        let fw_progress = self.firmware.progress;
        let fw_status = self.firmware.status.clone();

        let b: Option<bool> =
            if let (Some(cur), Some(latest)) = (self.firmware.current_version, &latest_str) {
                let latest_int = u16::from_str_radix(latest.trim_start_matches("0x"), 16).unwrap();
                Some(latest_int > cur)
            } else {
                None
            };

        let c = self.theme.colors.clone();
        ds_section(ui, &c, "Controller");
        ds_row(ui, |ui| {
            ds_label(ui, "Model");
            ds_value_text_lr(ui, model);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Serial");
            ds_value_text_lr(ui, &serial);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Build Date");
            ds_value_text_lr(ui, &build_date);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Build Time");
            ds_value_text_lr(ui, &build_time);
        });

        ds_section(ui, &c, "Firmware");
        ds_row(ui, |ui| {
            ds_label(ui, "Current");
            ds_value_text_lr(ui, &cur_str);
        });
        ds_row(ui, |ui| {
            ds_label(ui, "Latest");
            if checking {
                ui.spinner();
                ui.label(RichText::new("Checking…").size(18.0).color(c.text_dim()));
            } else if let Some(ver) = latest_str.as_ref() {
                ds_value_text_lr(ui, ver);
            } else if connected {
                if ds_pill_button(ui, &c, "Check", false).clicked() {
                    self.fetch_latest_verision_async();
                }
            } else {
                ds_value_text(ui, "-");
            }
        });
        if connected {
            ds_row(ui, |ui| {
                ds_label(ui, "Refresh");
                if ds_pill_button(ui, &c, "↻ Re-read info", false).clicked() {
                    self.refresh_firmware_info();
                }
            });
        }
        if let Some(needs_update) = b {
            ds_row(ui, |ui| {
                ds_label(ui, "Status");
                let (txt, col) = if needs_update {
                    ("Update available", c.warning())
                } else {
                    ("Up to date", c.success())
                };
                ui.label(RichText::new(txt).size(18.0).strong().color(col));
            });
        }

        if fw_updating {
            ds_section(ui, &c, "Flashing");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(ROW_PAD_X);
                ui.vertical(|ui| {
                    ui.label(RichText::new(&fw_status).color(c.text_dim()).size(14.0));
                    ui.add_space(8.0);
                    ui.add(
                        ProgressBar::new(fw_progress as f32 / 100.0)
                            .text(format!("{}%", fw_progress))
                            .animate(true),
                    );
                });
            });
        } else if let Some(true) = b {
            ds_section(ui, &c, "Update");
            ds_row(ui, |ui| {
                ds_label(ui, "Action");
                let can_flash = connected && !is_bt;
                let ota = ds_pill_button(ui, &c, "Download & Update", false);
                ui.add_space(8.0);
                let file = ds_pill_button(ui, &c, "Update from File…", false);
                if can_flash && ota.clicked() {
                    self.flash_latest();
                }
                if can_flash && file.clicked() {
                    self.flash_file();
                }
            });
            ds_row(ui, |ui| {
                ds_label(ui, "Notice");
                ui.label(
                    RichText::new(
                        "Do not disconnect during update // battery ≥ 10% // takes several minutes",
                    )
                    .size(15.0)
                    .color(c.warning()),
                );
            });
            if connected && is_bt {
                ds_row(ui, |ui| {
                    ds_label(ui, "Bluetooth");
                    ui.label(
                        RichText::new("Switch to USB to flash")
                            .size(18.0)
                            .color(c.error()),
                    );
                });
            }
        }
    }

    pub(crate) fn render_advanced(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.render_firmware_panel(ui);
            });
    }
}
