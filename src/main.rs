use std::{sync::{mpsc::{self, Receiver}, Arc, Mutex}, thread, time::{Duration, Instant}};

use eframe::App;
use egui::{include_image, pos2, vec2, Align2, Button, CentralPanel, Color32, Context, CornerRadius, Frame, Image, Layout, Margin, Pos2, ProgressBar, RichText, Sense, SidePanel, Slider, Ui};
use hidapi::HidApi;

use crate::{
    daemon::DaemonManager,
    dualsense::{BatteryInfo, DualSense, MicLedState},
    firmware::{get_product_name, FirmwareDownloader},
    profiles::{Profile, ProfileManager, SensitivityCurve},
    constants::*
};

mod dualsense;
mod firmware;
mod profiles;
mod daemon;
mod constants;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "DS4U",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(DS4UApp::new()))
        })
    )
}

#[derive(Debug, Clone)]
enum ProgressUpdate {
    Progress(u32),
    Status(String),
    Complete,
    Error(String),
    LatestVersion(String)
}

#[derive(PartialEq, Clone, Copy)]
enum TriggerMode {
    Off,
    Feedback
}

#[derive(PartialEq)]
enum Section {
    Lightbar,
    Triggers,
    Sticks,
    Audio,
    Advanced
}

struct LightbarState {
    r: f32,
    g: f32,
    b: f32,
    brightness: f32,
    enabled: bool
}

struct MicrophoneState {
    enabled: bool,
    led_state: MicLedState
}

struct TriggerState {
    mode: TriggerMode,
    position: u8,
    strength: u8
}

struct StickSettings {
    left_curve: SensitivityCurve,
    right_curve: SensitivityCurve,
    left_deadzone: f32,
    right_deadzone: f32,
}

struct DS4UApp {
    api: HidApi,
    controller: Option<Arc<Mutex<DualSense>>>,

    last_connection_check: Instant,

    active_section: Section,
    show_profiles_panel: bool,

    profile_manager: ProfileManager,
    current_profile: Option<Profile>,
    profile_edit_name: String,

    daemon_manager: DaemonManager,

    battery_info: Option<BatteryInfo>,
    last_battery_update: Instant,
    
    lightbar: LightbarState,
    player_leds: u8,
    microphone: MicrophoneState,
    triggers: TriggerState,
    sticks: StickSettings,

    firmware_downloader: FirmwareDownloader,
    firmware_progress_rx: Option<Receiver<ProgressUpdate>>,
    firmware_progress: u32,
    firmware_status: String,
    firmware_updating: bool,

    controller_serial: Option<String>,
    firmware_current_version: Option<u16>,
    firmware_latest_version: Option<String>,
    firmware_checking_latest: bool,
    firmware_build_date: Option<String>,
    firmware_build_time: Option<String>,

    status_message: String,
    error_message: String
}

impl DS4UApp {
    fn new() -> Self {
        let api = HidApi::new().unwrap();

        let mut app = Self {
            api,
            controller: None,
            last_connection_check: Instant::now(),
            
            active_section: Section::Lightbar,
            show_profiles_panel: false,

            profile_manager: ProfileManager::new(),
            current_profile: None,
            profile_edit_name: String::new(),

            daemon_manager: DaemonManager::new(),

            battery_info: None,
            last_battery_update: Instant::now() - Duration::from_secs(10),

            lightbar: LightbarState {
                r: 0.0, g: 0.5, b: 1.5, brightness: 255.0, enabled: true
            },

            player_leds: 1,

            microphone: MicrophoneState {
                enabled: false, led_state: MicLedState::Off
            },

            triggers: TriggerState {
                mode: TriggerMode::Off, position: 0, strength: 5
            },

            sticks: StickSettings {
                left_curve: SensitivityCurve::Default,
                right_curve: SensitivityCurve::Default,
                left_deadzone: 0.1,
                right_deadzone: 0.1
            },

            firmware_downloader: FirmwareDownloader::new(),
            firmware_progress_rx: None,
            firmware_progress: 0,
            firmware_status: String::new(),
            firmware_updating: false,

            controller_serial: None,
            firmware_current_version: None,
            firmware_latest_version: None,
            firmware_checking_latest: false,
            firmware_build_date: None,
            firmware_build_time: None,

            status_message: String::new(),
            error_message: String::new()
        };

        app.check_for_controller();
        app
    }

    fn connect_controller(&mut self) {
        match DualSense::new(&self.api, None) {
            Ok(ds) => {
                if let Ok((version, build_date, build_time)) = ds.get_firmware_info() {
                    self.firmware_current_version = Some(version);
                    self.firmware_build_date = Some(build_date);
                    self.firmware_build_time = Some(build_time);
                }
                self.controller_serial = Some(ds.serial().to_string());
                self.controller = Some(Arc::new(Mutex::new(ds)));
                self.firmware_latest_version = None;
                self.status_message = "Controller connected".to_string();
                self.error_message.clear();
                self.update_battery();
            }
            Err(e) => {
                self.error_message = format!("Failed to connect: {}", e);
                self.controller = None;
            }
        }
    }

    fn disconnect_controller(&mut self) {
        self.controller = None;
        self.battery_info = None;
        self.status_message = "Controller disconnected".to_string();
    }

    fn update_battery(&mut self) {
        if let Some(controller) = &self.controller 
            && let Ok(mut ctrl) = controller.lock() {
                if let Ok(info) = ctrl.get_battery() {
                    self.battery_info = Some(info);
                    self.last_battery_update = Instant::now();
                    return;
                }
        } else {
            return;
        }

        self.disconnect_controller();
    }

    fn check_firmware_progress(&mut self) {
        if let Some(rx) = &self.firmware_progress_rx {
            while let Ok(update) = rx.try_recv() {
                match update {
                    ProgressUpdate::Progress(p) => { self.firmware_progress = p; }
                    ProgressUpdate::Status(s) => { self.firmware_status = s; }
                    ProgressUpdate::Complete => {
                        self.firmware_updating = false;
                        self.status_message = "Firmware update completed".to_string();
                        self.firmware_progress = 100;
                    }
                    ProgressUpdate::Error(e) => {
                        self.firmware_updating = false;
                        self.firmware_checking_latest = false;
                        self.error_message = e;
                        self.firmware_progress = 0;
                    }
                    ProgressUpdate::LatestVersion(v) => {
                        self.firmware_latest_version = Some(v);
                        self.firmware_checking_latest = false;
                    }
                }
            }
        }
    }

    fn apply_lightbar(&mut self) {
        if let Some(controller) = &self.controller 
        && let Ok(mut ctrl) = controller.lock() {
            let _ = ctrl.set_lightbar(
                    (self.lightbar.r * 255.0) as u8,
                    (self.lightbar.g * 255.0) as u8,
                    (self.lightbar.b * 255.0) as u8,
                    self.lightbar.brightness as u8
                );
        }
    }

    fn apply_player_leds(&mut self) {
        if let Some(controller) = &self.controller 
        && let Ok(mut ctrl) = controller.lock() {
            let _ = ctrl.set_player_leds(self.player_leds);
        }
    }

    fn apply_microphone(&mut self) {
        if let Some(controller) = &self.controller 
        && let Ok(mut ctrl) = controller.lock() {
            let _ = ctrl.set_mic(self.microphone.enabled);
            let _ = ctrl.set_mic_led(self.microphone.led_state);
        }
    }

    fn apply_trigger(&mut self) {
        if let Some(controller) = &self.controller 
        && let Ok(mut ctrl) = controller.lock() {
            let _ = match self.triggers.mode {
                TriggerMode::Off => ctrl.set_trigger_off(),
                TriggerMode::Feedback => {
                    let mut strengths = [0u8; 10];

                    for i in self.triggers.position..10 {
                        strengths[i as usize] = self.triggers.strength;
                    }

                    let mut active_zones: u16 = 0;
                    let mut strength_zones: u32 = 0;

                    for i in 0..10 {
                        if strengths[i] > 0 {
                            let strength_value = ((strengths[i] - 1) & 0x07) as u32;
                            strength_zones |= strength_value << (3 * i);
                            active_zones |= 1 << i;
                        }
                    }

                    let params = [
                        (active_zones & 0xff) as u8,
                        ((active_zones >> 8) & 0xff) as u8,
                        (strength_zones & 0xff) as u8,
                        ((strength_zones >> 8) & 0xff) as u8,
                        ((strength_zones >> 16) & 0xff) as u8,
                        ((strength_zones >> 24) & 0xff) as u8,
                        0, 0, 0, 0
                    ];

                    ctrl.set_trigger_effect(true, true, 0x21, &params)
                },
            };
        }
    }

    fn load_profile(&mut self, profile: &Profile) {
        self.lightbar.r = profile.lightbar_r;
        self.lightbar.g = profile.lightbar_g;
        self.lightbar.b = profile.lightbar_b;
        self.lightbar.brightness = profile.lightbar_brightness;

        self.player_leds = profile.player_leds;

        self.microphone.enabled = profile.mic_enabled;
    
        self.apply_lightbar();
        self.apply_player_leds();
        self.current_profile = Some(profile.clone());
    }

    fn check_controller_connection(&mut self) {
        if let Some(controller) = &self.controller 
            && controller.lock().unwrap().get_battery().is_err() {
                self.disconnect_controller(); 
        }
    }

    fn check_for_controller(&mut self) {
        let devices = dualsense::list_devices(&self.api);

        if !devices.is_empty() {
            self.connect_controller();
        }
    }

    fn curve_value(curve: &SensitivityCurve, t: f32) -> f32 {
        match curve {
            SensitivityCurve::Default => t,
            SensitivityCurve::Quick   => t.powf(0.5),
            SensitivityCurve::Precise => t.powf(2.2),
            SensitivityCurve::Steady  => t.powf(1.6),
            SensitivityCurve::Digital => if t > 0.5 { 1.0 } else { 0.0 },
            SensitivityCurve::Dynamic => {
                let t2 = t * 2.0;
                if t < 0.5 {
                    0.5 * t2 * t2
                } else {
                    1.0 - 0.5 * (2.0 - t2) * (2.0 - t2)
                }
            }
        }
    }

    fn fetch_latest_verision_async(&mut self) {
        if self.firmware_checking_latest {
            return;
        }
        
        let Some(ref ctrl) = self.controller else { return };
        let pid = ctrl.lock().unwrap().product_id();
        let (tx, rx) = mpsc::channel();
        let downloader = FirmwareDownloader::new();

        self.firmware_checking_latest = true;
        self.firmware_progress_rx = Some(rx);
        thread::spawn(move || {
            match downloader.get_latest_version() {
                Ok((ds_ver, dse_ver)) => {
                    let ver = if pid == DS_PID { ds_ver } else { dse_ver };
                    let _ = tx.send(ProgressUpdate::LatestVersion(ver));
                }
                Err(e) => {
                    let _ = tx.send(ProgressUpdate::Error(
                        format!("Version check failed: {}", e)
                    ));
                }
            }
        });
    }

    fn flash_latest(&mut self) {
        let Some(ref ctrl) = self.controller else { return };
        let pid = ctrl.lock().unwrap().product_id();
        let ctrl = Arc::clone(ctrl);

        let (tx, rx) = mpsc::channel();

        self.firmware_progress_rx = Some(rx);
        self.firmware_updating = true;
        self.firmware_progress = 0;
        self.firmware_status = "Downloading latest firmware...".to_string();

        thread::spawn(move || {
            let downloader = FirmwareDownloader::new();
            let tx_dl = tx.clone();
            
            let fw_data = match downloader.download_latest_firmware(pid, move |p| {
                let _ = tx_dl.send(ProgressUpdate::Progress(p / 2));
            }) {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.send(ProgressUpdate::Error(e.to_string()));
                    return;
                }
            };

            let _ = tx.send(ProgressUpdate::Status("Flashing...".to_string()));
            let tx_flash = tx.clone();
            let result = ctrl.lock().unwrap().update_firmware(&fw_data, move |p| {
                let _ = tx_flash.send(ProgressUpdate::Progress(50 + p / 2));
            });

            match result {
                Ok(_) => { let _ = tx.send(ProgressUpdate::Complete); }
                Err(e) => { let _ = tx.send(ProgressUpdate::Error(e.to_string())); }
            }
        });
    }

    fn flash_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Select firmware file")
            .add_filter("Firmware binary", &["bin"])
            .pick_file()
        else { return };

        let fw_data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.error_message = format!("Failed to read file: {}", e);
                return;
            }
        };

        let Some(ref ctrl) = self.controller else {
            self.error_message = "No controller connected".to_string();
            return;
        };
        let ctrl = Arc::clone(ctrl);

        let (tx, rx) = mpsc::channel();

        self.firmware_progress_rx = Some(rx);
        self.firmware_updating = true;
        self.firmware_progress = 0;
        self.firmware_status = "Flasing from file...".to_string();

        thread::spawn(move || {
            let mut ctrl = ctrl.lock().unwrap();
            let tx_progress = tx.clone();
            
            let result = ctrl.update_firmware(&fw_data, move |p| {
                let _ = tx_progress.send(ProgressUpdate::Progress(p));
            });

            match result {
                Ok(_)  => { let _ = tx.send(ProgressUpdate::Complete); }
                Err(e) => { let _ = tx.send(ProgressUpdate::Error(e.to_string())); }
            }
        });
    }

    //
    // RENDER/UI
    //
    
    fn render_nav_btn(&mut self, ui: &mut Ui, label: &str, section: Section) {
        let is_active = self.active_section == section;

        let btn = Button::new(RichText::new(label).size(14.0))
            .fill(if is_active {
                Color32::from_rgb(0, 112, 220)
            } else {
                Color32::TRANSPARENT
            })
            .stroke(egui::Stroke::NONE)
            .min_size(vec2(ui.available_width(), 40.0));

        if ui.add(btn).clicked() {
            self.active_section = section;
        }
    }

    fn render_firmware_panel(&mut self, ui: &mut Ui) {
        ui.label(RichText::new("Firmware").size(18.0).strong());
        
        ui.add_space(14.0);

        let connected = self.controller.is_some();

        let is_bt = self.controller.as_ref()
            .map(|c| c.lock().unwrap().is_bluetooth())
            .unwrap_or(false);

        let model = self.controller.as_ref()
            .map(|c| get_product_name(c.lock().unwrap().product_id()))
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
            let cur_str = format!("{:04X}", cur);
            let needs_update = latest.replace(".", "")
                .trim_start_matches('0').to_uppercase() !=
                cur_str.trim_start_matches('0').to_uppercase();
            Some(needs_update)
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
        } else {
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

    fn render_advanced(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Advanced Settings").size(28.0));
        ui.add_space(30.0);

        self.render_firmware_panel(ui);
    }

    fn render_connection_status(&mut self, ui: &mut Ui) {
        Frame::NONE
            .fill(Color32::from_rgb(20, 30, 50))
            .corner_radius(CornerRadius::same(12))
            .inner_margin(Margin::same(12))
            .show(ui, |ui| {
                if self.controller.is_some() {
                    ui.label(RichText::new("Connected")
                        .size(12.0)
                        .color(Color32::WHITE));

                    if let Some(battery) = &self.battery_info {
                        ui.add_space(10.0);
                        ui.horizontal(|ui| {
                            ui.label(format!("{}%", battery.capacity));

                            let battery_color = if battery.capacity > 50 {
                                Color32::from_rgb(0, 200, 100)
                            } else if battery.capacity > 20 {
                                Color32::from_rgb(255, 180, 0)
                            } else {
                                Color32::from_rgb(255, 50, 50)
                            };

                            let bar_width = ui.available_width();
                            let (rect, _) = ui.allocate_exact_size(
                                    vec2(bar_width, 4.0),
                                    egui::Sense::hover()
                            );

                            ui.painter().rect_filled(
                                egui::Rect::from_min_size(
                                    rect.min,
                                    vec2(bar_width * 
                                        (battery.capacity as f32 / 100.0), 4.0)
                                ),
                                2.0,
                                battery_color
                            );
                        });
                    }
                } else { 
                    let spinner = egui::Spinner::new()
                        .size(12.0)
                        .color(Color32::from_rgb(0, 112, 220));

                    ui.add(spinner);

                    ui.label(RichText::new("Searching...")
                        .size(12.0)
                        .color(Color32::from_rgb(0, 112, 220)));
                }
            });
    }

    fn render_sidebar(&mut self, ui: &mut Ui) {
        ui.add_space(20.0);

        ui.with_layout(Layout::top_down(egui::Align::Center), |ui| {
            ui.label(RichText::new("DS4U🇺🇦").size(24.0)
                .color(Color32::WHITE).strong());

            self.render_connection_status(ui);
        });

        ui.add_space(5.0);
        ui.separator();
        ui.add_space(20.0);

        if self.controller.is_some() {
            ui.label(RichText::new("Profile")
                .size(12.0)
                .color(Color32::GRAY));

            ui.add_space(5.0);

            egui::ComboBox::from_id_salt("profile_combo")
                .selected_text(self.current_profile.as_ref()
                    .map(|p| p.name.as_str())
                    .unwrap_or("Default"))
                .width(ui.available_width())
                .show_ui(ui, |ui| {
                    if ui.selectable_label
                        (self.current_profile.is_none(), "Default").clicked() {
                        self.current_profile = None;
                    }

                    for profile in self.profile_manager.list_profiles() {
                        if ui.selectable_label(
                                self.current_profile.as_ref()
                                    .map(|p| &p.name) == Some(&profile.name),
                                &profile.name)
                            .clicked() {
                                self.load_profile(&profile);
                        }
                    }
                });

            ui.add_space(10.0);

            if ui.button("Manage Profiles").clicked() {
                self.show_profiles_panel = !self.show_profiles_panel;
            }

            ui.add_space(30.0);
            ui.separator();
            ui.add_space(20.0);

            self.render_nav_btn(ui, "Lightbar", Section::Lightbar);
            self.render_nav_btn(ui, "Triggers", Section::Triggers);
            self.render_nav_btn(ui, "Sticks", Section::Sticks);
            self.render_nav_btn(ui, "Audio", Section::Audio);
            self.render_nav_btn(ui, "Advanced", Section::Advanced);
        }

        ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
            ui.add_space(10.0);

            if !self.error_message.is_empty() {
                ui.label(RichText::new(&self.error_message)
                    .size(11.0)
                    .color(Color32::from_rgb(255, 100, 100)));
            }

            if !self.status_message.is_empty() {
                ui.label(RichText::new(&self.status_message)
                    .size(11.0)
                    .color(Color32::from_rgb(100, 255, 100)));
            }
        });
    }

    fn render_audio_settings(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Microphone & Audio").size(28.0));
        ui.add_space(10.0);

        ui.label(RichText::new("Configure microphone")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.checkbox(&mut self.microphone.enabled, "Microphone Enabled");

        ui.add_space(20.0);

        ui.label("LED:");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.microphone.led_state, MicLedState::Off, "Off");
            ui.selectable_value(&mut self.microphone.led_state, MicLedState::On, "On");
            ui.selectable_value(&mut self.microphone.led_state, MicLedState::Pulse, "Pulse");
        });

        if ui.button("Apply").clicked() {
            self.apply_microphone();
        }
    }

    fn render_stick_visual(ui: &mut Ui, deadzone: f32) {
        let size = 120.0;
        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();
        let center = rect.center();
        let radius = size / 2.0;

        painter.circle_stroke(
            center,
            radius - 1.0,
            egui::Stroke::new(2.0, Color32::from_rgb(50, 70, 100))
        );
        
        painter.circle_filled(
            center,
            radius - 2.0,
            Color32::from_rgb(12, 18, 30)
        );

        let dz_radius = deadzone / 0.3 * (radius - 4.0);
        
        painter.circle_filled(
            center,
            dz_radius,
            Color32::from_rgba_unmultiplied(220, 60, 60, 40)
        );

        painter.circle_stroke(
            center,
            dz_radius,
            egui::Stroke::new(1.0, Color32::from_rgb(200, 60, 60))
        );
        
        painter.circle_filled(
            center,
            4.0,
            Color32::from_rgb(0, 122, 250)
        );
    }

    fn render_curve_visual(ui: &mut Ui, curve: &SensitivityCurve, deadzone: f32) {
        let size = 140.0;
        let pad = 12.0;

        let (rect, _) = ui.allocate_exact_size(vec2(size, size), Sense::hover());
        let painter = ui.painter();

        painter.rect_filled(
            rect,
            6.0,
            Color32::from_rgb(10, 16, 26)
        );
        
        painter.rect_stroke(
            rect,
            6.0,
            egui::Stroke::new(1.5, Color32::from_rgb(40, 60, 90)),
            egui::StrokeKind::Outside
        );

        let plot_rect = egui::Rect::from_min_size(
            pos2(rect.min.x + pad, rect.min.y + pad),
            vec2(size - pad * 2.0, size - pad * 2.0)
        );

        for t in [0.25, 0.5, 0.75] {
            let x = plot_rect.min.x + t * plot_rect.width();
            let y = plot_rect.min.y + t * plot_rect.height();
            
            painter.line_segment(
                [pos2(x, plot_rect.min.y), pos2(x, plot_rect.max.y)],
                egui::Stroke::new(0.5, Color32::from_rgb(25, 40, 60))
            );

            painter.line_segment(
                [pos2(plot_rect.min.x, y), pos2(plot_rect.max.x, y)],
                egui::Stroke::new(0.5, Color32::from_rgb(25, 40, 60))
            );
        }

        painter.line_segment(
            [plot_rect.left_bottom(), plot_rect.right_top()],
            egui::Stroke::new(1.0, Color32::from_rgb(40, 60, 80))
        );

        let dz_x = plot_rect.min.x + deadzone / 0.3 * plot_rect.width() * 0.3;

        painter.rect_filled(
            egui::Rect::from_min_max(
                plot_rect.min,
                pos2(dz_x, plot_rect.max.y)
            ),
            0.0,
            Color32::from_rgba_unmultiplied(200, 50, 50, 25)
        );

        let steps = 80;
        let mut points: Vec<Pos2> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let out = Self::curve_value(curve, t);
            let x = plot_rect.min.x + t * plot_rect.width();
            let y = plot_rect.max.y - out * plot_rect.height();
            points.push(pos2(x, y));
        }

        let accent = Color32::from_rgb(0, 150, 255);
        for w in points.windows(2) {
            painter.line_segment([w[0], w[1]], egui::Stroke::new(2.0, accent));
        }

        let font = egui::FontId::proportional(9.0);
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 3.0),
            Align2::RIGHT_TOP, "0",
            font.clone(),
            Color32::from_gray(80)
        );
        painter.text(
            plot_rect.left_bottom() + vec2(2.0, 3.0),
            Align2::LEFT_TOP, "1",
            font.clone(),
            Color32::from_gray(80)
        );
        painter.text(
            plot_rect.left_bottom() + vec2(-2.0, 0.0),
            Align2::RIGHT_CENTER, "1",
            font.clone(),
            Color32::from_gray(80)
        );
    }

    fn render_sticks_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Stick Sensitivity").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Adjust response curves and deadzones")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.columns(2, |cols| {
            cols[0].label(RichText::new("Left Stick").size(16.0).strong());
            cols[0].add_space(10.0);

            egui::ComboBox::from_id_salt("left_curve")
                .selected_text(format!("{:?}", self.sticks.left_curve))
                .width(cols[0].available_width())
                .show_ui(&mut cols[0], |ui| {
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Default,
                        "Default"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Quick,
                        "Quick"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Precise,
                        "Precise"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Steady,
                        "Steady"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Dynamic,
                        "Dynamic"
                    );
                    ui.selectable_value(
                        &mut self.sticks.left_curve,
                        SensitivityCurve::Digital,
                        "Digital"
                    );
                });

            Self::render_curve_visual(
                &mut cols[0],
                &self.sticks.left_curve,
                self.sticks.left_deadzone
            );

            cols[0].add_space(15.0);
            cols[0].label("Deadzone");
            cols[0].add(Slider::new(&mut self.sticks.left_deadzone, 0.0..=0.3));
            Self::render_stick_visual(&mut cols[0], self.sticks.left_deadzone);

            cols[1].label(RichText::new("Right Stick").size(16.0).strong());
            cols[1].add_space(10.0);

            egui::ComboBox::from_id_salt("right_curve")
                .selected_text(format!("{:?}", self.sticks.right_curve))
                .width(cols[0].available_width())
                .show_ui(&mut cols[1], |ui| {
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Default,
                        "Default"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Quick,
                        "Quick"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Precise,
                        "Precise"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Steady,
                        "Steady"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Dynamic,
                        "Dynamic"
                    );
                    ui.selectable_value(
                        &mut self.sticks.right_curve,
                        SensitivityCurve::Digital,
                        "Digital"
                    );
                });

            Self::render_curve_visual(
                &mut cols[1],
                &self.sticks.right_curve,
                self.sticks.right_deadzone
            );

            cols[1].add_space(15.0);
            cols[1].label("Deadzone");
            cols[1].add(Slider::new(&mut self.sticks.right_deadzone, 0.0..=0.3));
            Self::render_stick_visual(&mut cols[1], self.sticks.right_deadzone);
        });
    }

    fn render_triggers_section(&mut self, ui: &mut Ui) { 
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

    fn render_lightbar_section(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Lightbar & Indicators").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Customize your controller lights")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

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

    fn render_main(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.add_space(30.0);

            match self.active_section {
                Section::Lightbar => self.render_lightbar_section(ui),
                Section::Triggers => self.render_triggers_section(ui),
                Section::Sticks   => self.render_sticks_section(ui),
                Section::Audio    => self.render_audio_settings(ui),
                Section::Advanced => self.render_advanced(ui),
            }

            ui.add_space(30.0);
        });
    }
    
    fn render_connection(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            let time = ui.input(|i| i.time);
            let pulse = (time * 2.0).sin() * 0.3 + 0.7;
            let alpha = (pulse * 255.0) as u8;

            let controller_pic = Image::new(include_image!("../assets/controller.svg"))
                .maintain_aspect_ratio(true)
                .max_width(350.0);

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

            ui.add_space(40.0);

            if ui.button(RichText::new("Refresh").size(14.0))
                .clicked() {
                self.check_for_controller();
            }

        });
    }
}

impl App for DS4UApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.controller.is_none() {
            ctx.request_repaint_after_secs(1.0);
            if self.last_connection_check.elapsed() > Duration::from_secs(1) {
            self.check_for_controller();
            self.last_connection_check = Instant::now();
            }
        } else {
            ctx.request_repaint_after_secs(5.0);
            if self.last_battery_update.elapsed() > Duration::from_secs(5) {
                self.update_battery();
            }
        }

        self.check_firmware_progress();

        apply_style(ctx);

        SidePanel::left("sidebar")
            .exact_width(280.0)
            .resizable(false)
            .show(ctx, |ui| {
                self.render_sidebar(ui);
            });

        CentralPanel::default().show(ctx, |ui| {
            if self.controller.is_some() {
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

fn apply_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals.dark_mode = true;
    style.visuals.window_fill = Color32::from_rgb(12, 18, 28);
    style.visuals.panel_fill = Color32::from_rgb(16, 24, 36);
    style.visuals.extreme_bg_color = Color32::from_rgb(8, 12, 20);

    let accent_color = Color32::from_rgb(0, 122, 250);
    style.visuals.selection.bg_fill = accent_color;
    style.visuals.widgets.active.bg_fill = accent_color;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(40, 60, 90);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 45, 70);

    style.visuals.window_corner_radius = CornerRadius::same(8);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(6);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(6);

    style.spacing.item_spacing = vec2(10.0, 10.0);
    style.spacing.button_padding = vec2(16.0, 8.0);

    ctx.set_style(style);
}

