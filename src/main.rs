use std::{sync::{self, atomic::Ordering, mpsc::{self, Receiver}, Arc, Mutex}, thread::{self, sleep}, time::{Duration, Instant}};

use eframe::App;
use egui::{include_image, pos2, vec2, Align2, Button, CentralPanel, Color32, Context, CornerRadius, Frame, Image, Layout, Margin, Painter, Pos2, ProgressBar, RichText, Sense, SidePanel, Slider, Ui};
use hidapi::HidApi;

use crate::{
    daemon::DaemonManager,
    dualsense::{BatteryInfo, DualSense, MicLedState},
    firmware::{get_product_name, FirmwareDownloader},
    profiles::{Profile, ProfileManager},
    common::*,
    inputs::*
};

mod dualsense;
mod firmware;
mod profiles;
mod daemon;
mod common;
mod inputs;

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

#[derive(PartialEq)]
enum Section {
    Lightbar,
    Triggers,
    Sticks,
    Haptics,
    Audio,
    Advanced,
    Inputs
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

struct AudioSettings {
    volume: u8,
    speaker_mode: SpeakerMode
}

struct VibrationSettings {
    rumble: u8,
    trigger: u8
}

struct DS4UApp {
    api: HidApi,
    controller: Option<Arc<Mutex<DualSense>>>,

    last_connection_check: Instant,

    active_section: Section,
    show_profiles_panel: bool,

    controller_is_bt: Option<bool>,
    controller_product_id: Option<u16>,

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
    audio: AudioSettings,
    vibration: VibrationSettings,

    firmware_downloader: FirmwareDownloader,
    firmware_progress_rx: Option<Receiver<ProgressUpdate>>,
    firmware_progress: u32,
    firmware_status: String,
    firmware_updating: bool,
    update_mode_flag: Option<Arc<sync::atomic::AtomicBool>>,
    controller_serial: Option<String>,
    firmware_current_version: Option<u16>,
    firmware_latest_version: Option<String>,
    firmware_checking_latest: bool,
    firmware_build_date: Option<String>,
    firmware_build_time: Option<String>,

    status_message: String,
    error_message: String,

    controller_state: Option<ControllerState>,
    input_state_rx: Option<mpsc::Receiver<ControllerState>>,
    input_polling: bool,
    input_poll_stop: Option<Arc<sync::atomic::AtomicBool>>,

    pending_connect_since: Option<Instant>,
}

impl DS4UApp {
    fn new() -> Self {
        let api = HidApi::new().unwrap();

        let mut app = Self {
            api,
            controller: None,
            last_connection_check: Instant::now(),
            
            active_section: Section::Inputs,
            show_profiles_panel: false,

            controller_is_bt: None,
            controller_product_id: None,

            profile_manager: ProfileManager::new(),
            current_profile: None,
            profile_edit_name: String::new(),

            daemon_manager: DaemonManager::new(),

            battery_info: None,
            last_battery_update: Instant::now() - Duration::from_secs(10),

            lightbar: LightbarState {
                r: 0.0,
                g: 0.5,
                b: 1.0,
                brightness: 255.0,
                enabled: true
            },

            player_leds: 1,

            microphone: MicrophoneState {
                enabled: false,
                led_state: MicLedState::Off
            },

            triggers: TriggerState {
                mode: TriggerMode::Off,
                position: 0,
                strength: 5
            },

            sticks: StickSettings {
                left_curve: SensitivityCurve::Default,
                right_curve: SensitivityCurve::Default,
                left_deadzone: 0.1,
                right_deadzone: 0.1
            },

            audio: AudioSettings {
                volume: 0,
                speaker_mode: SpeakerMode::Internal
            },

            vibration: VibrationSettings {
                rumble: 0,
                trigger: 0
            },

            firmware_downloader: FirmwareDownloader::new(),
            firmware_progress_rx: None,
            firmware_progress: 0,
            firmware_status: String::new(),
            firmware_updating: false,

            update_mode_flag: None,

            controller_serial: None,
            firmware_current_version: None,
            firmware_latest_version: None,
            firmware_checking_latest: false,
            firmware_build_date: None,
            firmware_build_time: None,

            status_message: String::new(),
            error_message: String::new(),

            controller_state: None,
            input_state_rx: None,
            input_polling: false,
            input_poll_stop: None,

            pending_connect_since: None
        };

        app.check_for_controller();
        app
    }

    fn start_input_polling(&mut self) {
        let Some(ctrl) = self.controller.clone() else { return };

        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(sync::atomic::AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        self.input_state_rx = Some(rx);
        self.input_poll_stop = Some(stop_flag);
        self.input_polling = true;

        thread::spawn(move || {
            while !stop_clone.load(sync::atomic::Ordering::Relaxed) {
                if let Ok(mut c) = ctrl.try_lock() {
                    if let Ok(state) = c.get_input_state() {
                        let _ = tx.send(state);
                    }
                    drop(c);
                } else {
                    sleep(Duration::from_millis(8));
                }
            }
        });
    }

    fn stop_input_polling(&mut self) {
        if let Some(flag) = &self.input_poll_stop {
            flag.store(true, sync::atomic::Ordering::Relaxed);
        }

        self.input_poll_stop = None;
        self.input_state_rx = None;
        self.input_polling = false;
        self.controller_state = None;
    }

    fn connect_controller(&mut self) {
        match DualSense::new(&self.api, None) {
            Ok(ds) => {
                if let Ok((version, build_date, build_time)) = ds.get_firmware_info() {
                    self.firmware_current_version = Some(version);
                    self.firmware_build_date = Some(build_date);
                    self.firmware_build_time = Some(build_time);
                } else {
                    self.firmware_current_version = None;
                    self.firmware_build_date = None;
                    self.firmware_build_time = None;
                }

                self.controller_serial = Some(ds.serial().to_string());
                self.controller_is_bt = Some(ds.is_bluetooth());
                self.controller_product_id = Some(ds.product_id());
                self.controller = Some(Arc::new(Mutex::new(ds)));
                self.firmware_latest_version = None;
                self.status_message = "Controller connected".to_string();
                self.error_message.clear();
                self.lightbar.enabled = true;
                self.update_battery();
            }
            Err(_) => {
                self.controller = None;
            }
        }
    }

    fn disconnect_controller(&mut self) {
        self.controller = None;
        self.battery_info = None;
        self.controller_is_bt = None;
        self.controller_product_id = None;
        self.status_message = "Controller disconnected".to_string();
    }

    fn update_battery(&mut self) {
        self.last_battery_update = Instant::now();
        let in_update_mode = self.update_mode_flag
            .as_ref()
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false);

        if in_update_mode {
            return;
        }

        if self.update_mode_flag.is_some() {
            self.update_mode_flag = None;
        }

        let Some(controller) = &self.controller else { return };

        let Ok(mut ctrl) = controller.try_lock() else { return };

        if let Ok(info) = ctrl.get_battery() {
            self.battery_info = Some(info);
        } else {
            drop(ctrl);
            self.disconnect_controller();
        }
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
                        self.update_mode_flag = None;
                    }
                    ProgressUpdate::Error(e) => {
                        self.firmware_updating = false;
                        self.firmware_checking_latest = false;
                        self.error_message = e;
                        self.firmware_progress = 0;
                        self.update_mode_flag = None;
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

    fn apply_lightbar_enable(&mut self) {
        if let Some(controller) = &self.controller 
        && let Ok(mut ctrl) = controller.lock() {
            let _ = ctrl.set_lightbar_enabled(self.lightbar.enabled);
        }
        if self.lightbar.enabled {
            self.apply_lightbar();
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
                TriggerMode::Bow | TriggerMode::Weapon | TriggerMode::Machine | TriggerMode::Galloping | TriggerMode::Vibration => todo!()
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
        let in_update_mode = self.update_mode_flag
            .as_ref()
            .map(|f| f.load(Ordering::Relaxed))
            .unwrap_or(false);

        if in_update_mode {
            return;
        }

        let Some(serial) = self.controller_serial.clone() else { return };

        if self.api.refresh_devices().is_err() {
            return;
        }

        let still_present = self.api.device_list().any(|info| {
            info.vendor_id() == DS_VID
                && (info.product_id() == DS_PID || info.product_id() == DSE_PID)
                && info.serial_number() == Some(serial.as_str())
        });

        if !still_present {
            self.stop_input_polling();
            self.disconnect_controller();
        }
    }

    fn check_for_controller(&mut self) {
        self.last_connection_check = Instant::now();
     
        if self.api.refresh_devices().is_err() {
            return;
        }

        if self.controller.is_none() && !dualsense::list_devices(&self.api).is_empty() {
            match self.pending_connect_since {
                None => { 
                    self.pending_connect_since = Some(Instant::now());
                }
                Some(since) if since.elapsed() >= Duration::from_millis(400) => {
                    self.pending_connect_since = None;
                    self.connect_controller();
                }
                _ => {}
            }
        } else if self.controller.is_none() {
            self.pending_connect_since = None;
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
        let downloader = self.firmware_downloader.clone();

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
        self.stop_input_polling();
        
        let Some(ref ctrl) = self.controller else { return };
        let pid = self.controller_product_id
            .unwrap_or_else(|| ctrl.lock().unwrap().product_id());

        let update_mode_flag = ctrl.lock().unwrap().update_mode_flag();
        self.update_mode_flag = Some(Arc::clone(&update_mode_flag));

        let ctrl = Arc::clone(ctrl);

        let (tx, rx) = mpsc::channel();

        self.firmware_progress_rx = Some(rx);
        self.firmware_updating = true;
        self.firmware_progress = 0;
        self.firmware_status = "Downloading latest firmware...".to_string();

        let downloader = self.firmware_downloader.clone();

        thread::spawn(move || {
            {
                let c = ctrl.lock().unwrap();
                c.set_update_mode(true);
            }

            let mut ctrl = ctrl.lock().unwrap();

            let tx_dl = tx.clone();
            
            let fw_data = match downloader.download_latest_firmware(pid, move |p| {
                let _ = tx_dl.send(ProgressUpdate::Progress(p / 2));
            }) {
                Ok(d) => d,
                Err(e) => {
                    ctrl.set_update_mode(false);
                    let _ = tx.send(ProgressUpdate::Error(e.to_string()));
                    return;
                }
            };

            let _ = tx.send(ProgressUpdate::Status("Flashing...".to_string()));
            let tx_flash = tx.clone();
            let result = ctrl.update_firmware(&fw_data, move |p| {
                let _ = tx_flash.send(ProgressUpdate::Progress(50 + p / 2));
            });

            ctrl.set_update_mode(false);

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

        self.stop_input_polling();

        let Some(ref ctrl) = self.controller else {
            self.error_message = "No controller connected".to_string();
            return;
        };

        let update_mode_flag = ctrl.lock().unwrap().update_mode_flag();
        self.update_mode_flag = Some(Arc::clone(&update_mode_flag));

        let ctrl = Arc::clone(ctrl);

        let (tx, rx) = mpsc::channel();

        self.firmware_progress_rx = Some(rx);
        self.firmware_updating = true;
        self.firmware_progress = 0;
        self.firmware_status = "Flasing from file...".to_string();

        thread::spawn(move || {
            {
                let c = ctrl.lock().unwrap();
                c.set_update_mode(true);
            }

            let mut ctrl = ctrl.lock().unwrap();
            let tx_progress = tx.clone();
            
            let result = ctrl.update_firmware(&fw_data, move |p| {
                let _ = tx_progress.send(ProgressUpdate::Progress(p));
            });

            ctrl.set_update_mode(false);

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

    fn render_haptics_settings(&mut self, ui: &mut Ui) {
        ui.heading(RichText::new("Haptic Settings").size(28.0));
        
        ui.add_space(10.0);

        ui.label(RichText::new("Configure vibration and haptic feedback")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        ui.label(RichText::new("Vibration").size(18.0).strong());

        ui.add_space(10.0);
        
        ui.label(RichText::new("Reduce haptic feedback strength (0 = full, 7 = minimum)")
            .size(12.0)
            .color(Color32::GRAY));

        let mut changed = false;

        ui.horizontal(|ui| {
            ui.label("Rumble Motors:");
            if ui.add(Slider::new(&mut self.vibration.rumble, 0..=7)
                .text("")).changed() { changed = true; }
            ui.label(format!("{}", self.vibration.rumble));
        });

        ui.add_space(10.0);
        
        ui.horizontal(|ui| {
            ui.label("Trigger Vibration:");
            if ui.add(Slider::new(&mut self.vibration.trigger, 0..=7)
                .text("")).changed() { changed = true; }
            ui.label(format!("{}", self.vibration.trigger));
        });

        if changed && let Some(controller) = &self.controller
            && let Ok(mut ctrl) = controller.lock() {
                let _ = ctrl.set_vibration(
                    self.vibration.rumble,
                    self.vibration.trigger
                );
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

                    if let Some(battery) = &self.battery_info {
                        ui.label(RichText::new(
                                format!("Connected • {}", battery.status)
                                )
                            .size(12.0)
                            .color(Color32::WHITE));
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
                    } else { 
                        ui.label(RichText::new("Connected")
                            .size(12.0)
                            .color(Color32::WHITE));
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
            // ui.label(RichText::new("Profile")
            //     .size(12.0)
            //     .color(Color32::GRAY));
            //
            // ui.add_space(5.0);
            //
            // egui::ComboBox::from_id_salt("profile_combo")
            //     .selected_text(self.current_profile.as_ref()
            //         .map(|p| p.name.as_str())
            //         .unwrap_or("Default"))
            //     .width(ui.available_width())
            //     .show_ui(ui, |ui| {
            //         if ui.selectable_label
            //             (self.current_profile.is_none(), "Default").clicked() {
            //             self.current_profile = None;
            //         }
            //
            //         for profile in self.profile_manager.list_profiles() {
            //             if ui.selectable_label(
            //                     self.current_profile.as_ref()
            //                         .map(|p| &p.name) == Some(&profile.name),
            //                     &profile.name)
            //                 .clicked() {
            //                     self.load_profile(&profile);
            //             }
            //         }
            //     });
            //
            // ui.add_space(10.0);
            //
            // if ui.button("Manage Profiles").clicked() {
            //     self.show_profiles_panel = !self.show_profiles_panel;
            // }
            //
            // ui.add_space(30.0);
            // ui.separator();
            // ui.add_space(20.0);

            self.render_nav_btn(ui, "Inputs", Section::Inputs);
            self.render_nav_btn(ui, "Lightbar", Section::Lightbar);
            self.render_nav_btn(ui, "Triggers", Section::Triggers);
            self.render_nav_btn(ui, "Sticks", Section::Sticks);
            self.render_nav_btn(ui, "Haptics", Section::Haptics);
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
                if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                    let _ = ctrl.set_speaker("internal");
                }
            }

            if ui.selectable_label(
                self.audio.speaker_mode == SpeakerMode::Headphone,
                "Headphone"
            ).clicked() {
                self.audio.speaker_mode = SpeakerMode::Headphone;
                if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                    let _ = ctrl.set_speaker("headphone");
                }
            }

            if ui.selectable_label(
                self.audio.speaker_mode == SpeakerMode::Both,
                "Both"
            ).clicked() {
                self.audio.speaker_mode = SpeakerMode::Both;
                if let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                    let _ = ctrl.set_speaker("both");
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
            if ui.add(Slider::new(&mut self.audio.volume, 0..=255)
                .text("")).changed()
                && let Some(controller) = &self.controller
                    && let Ok(mut ctrl) = controller.lock() {
                    let _ = ctrl.set_volume(self.audio.volume); 
            }
        });
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
            if ui.selectable_label(
                self.lightbar.enabled,
                "On"
            ).clicked() {
                self.lightbar.enabled = true;
                self.apply_lightbar_enable();
            }

            if ui.selectable_label(
                !self.lightbar.enabled,
                "Off"
            ).clicked() {
                self.lightbar.enabled = false;
                self.apply_lightbar_enable();
            }
        });      


        if self.lightbar.enabled {
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
        }
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

    fn render_inputs_section(&self, ui: &mut Ui) {
        ui.heading(RichText::new("Controller Inputs").size(28.0));

        ui.add_space(10.0);

        ui.label(RichText::new("Live visualisation")
            .size(14.0)
            .color(Color32::GRAY));

        ui.add_space(30.0);

        let state        = self.controller_state.as_ref();
        let buttons      = state.map_or(0,     |s| s.buttons);
        let dpad         = state.map_or(DPAD_NEUTRAL, |s| s.dpad);
        let l2_raw       = state.map_or(0u8,  |s| s.l2);
        let r2_raw       = state.map_or(0u8,  |s| s.r2);
        let lx           = state.map_or(0x80u8, |s| s.left_x);
        let ly           = state.map_or(0x80u8, |s| s.left_y);
        let rx_ax        = state.map_or(0x80u8, |s| s.right_x);
        let ry_ax        = state.map_or(0x80u8, |s| s.right_y);
        let touch_count  = state.map_or(0u8,  |s| s.touch_count);
        let touch_pts    = state.map(|s| &s.touch_points);

        let canvas_w = 700.0;
        let canvas_h = 360.0;

        let (canvas, _) = ui.allocate_exact_size(
            vec2(canvas_w, canvas_h),
            egui::Sense::hover(),
        );

        let p = ui.painter_at(canvas);
        let o = canvas.min;

        let px = |x: f32| o.x + x;
        let py = |y: f32| o.y + y;
        let pt = |x: f32, y: f32| pos2(o.x + x, o.y + y);

        let col_body       = Color32::from_rgb(28, 38, 58);
        let col_body_edge  = Color32::from_rgb(48, 65, 95);
        let col_btn_off    = Color32::from_rgb(38, 52, 78);
        let col_btn_edge   = Color32::from_rgb(55, 75, 110);
        let col_label      = Color32::from_rgb(140, 155, 180);
        let col_accent     = Color32::from_rgb(0, 122, 250);

        let col_triangle   = Color32::from_rgb(0,   180, 140);
        let col_circle     = Color32::from_rgb(210,  55,  55);
        let col_cross      = Color32::from_rgb(80,  140, 220);
        let col_square     = Color32::from_rgb(190,  80, 180);

        let col_dpad_active = Color32::from_rgb(200, 210, 230);
        let col_shoulder_active = col_accent;
        let col_system_active   = col_accent;

        p.rect_filled(
            egui::Rect::from_min_max(pt(60.0, 40.0), pt(640.0, 255.0)),
            CornerRadius::same(56),
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(60.0, 40.0), pt(640.0, 255.0)),
            CornerRadius::same(56),
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        p.rect_filled(
            egui::Rect::from_min_max(pt(82.0, 195.0), pt(218.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(82.0, 195.0), pt(218.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        p.rect_filled(
            egui::Rect::from_min_max(pt(482.0, 195.0), pt(618.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            col_body,
        );
        p.rect_stroke(
            egui::Rect::from_min_max(pt(482.0, 195.0), pt(618.0, 345.0)),
            CornerRadius { nw: 8, ne: 8, sw: 50, se: 50 },
            egui::Stroke::new(1.5, col_body_edge),
            egui::StrokeKind::Outside,
        );

        let l2_rect = egui::Rect::from_min_max(pt(62.0, 12.0), pt(202.0, 38.0));
        let r2_rect = egui::Rect::from_min_max(pt(498.0, 12.0), pt(638.0, 38.0));

        for rect in [l2_rect, r2_rect] {
            p.rect_filled(rect, CornerRadius::same(5), Color32::from_rgb(18, 26, 42));
            p.rect_stroke(rect, CornerRadius::same(5),
                egui::Stroke::new(1.0, col_body_edge),
                egui::StrokeKind::Outside);
        }

        let l2_fill_w = l2_rect.width() * (l2_raw as f32 / 255.0);
        if l2_fill_w > 0.0 {
            let fill = l2_rect.with_max_x(l2_rect.min.x + l2_fill_w);
            p.rect_filled(fill, CornerRadius::same(5), col_accent);
        }

        let r2_fill_w = r2_rect.width() * (r2_raw as f32 / 255.0);
        if r2_fill_w > 0.0 {
            let fill = r2_rect.with_min_x(r2_rect.max.x - r2_fill_w);
            p.rect_filled(fill, CornerRadius::same(5), col_accent);
        }

        p.text(pt(132.0, 25.0), Align2::CENTER_CENTER, "L2",
               egui::FontId::proportional(11.0), col_label);
        p.text(pt(568.0, 25.0), Align2::CENTER_CENTER, "R2",
               egui::FontId::proportional(11.0), col_label);

        let l1_pressed = buttons & BTN_L1 != 0;
        let r1_pressed = buttons & BTN_R1 != 0;

        let l1_rect = egui::Rect::from_min_max(pt(65.0, 40.0), pt(200.0, 62.0));
        let r1_rect = egui::Rect::from_min_max(pt(500.0, 40.0), pt(635.0, 62.0));

        p.rect_filled(l1_rect, CornerRadius { nw: 4, ne: 4, sw: 4, se: 4 },
            if l1_pressed { col_shoulder_active } else { col_btn_off });
        p.rect_filled(r1_rect, CornerRadius::same(4),
            if r1_pressed { col_shoulder_active } else { col_btn_off });

        p.text(pt(132.0, 51.0), Align2::CENTER_CENTER, "L1",
               egui::FontId::proportional(11.0), col_label);
        p.text(pt(568.0, 51.0), Align2::CENTER_CENTER, "R1",
               egui::FontId::proportional(11.0), col_label);

        let dc = pt(192.0, 152.0);
        let arm_w = 22.0;
        let arm_h = 26.0;
        let cr = CornerRadius::same(3);

        let dpad_rects = [
            (egui::Rect::from_center_size(
                pos2(dc.x,            dc.y - arm_h),
                vec2(arm_w, arm_h)), [DPAD_N, DPAD_NE, DPAD_NW], "▲"),
            (egui::Rect::from_center_size(
                pos2(dc.x,            dc.y + arm_h),
                vec2(arm_w, arm_h)), [DPAD_S, DPAD_SE, DPAD_SW], "▼"),
            (egui::Rect::from_center_size(
                pos2(dc.x - arm_h,   dc.y),
                vec2(arm_h, arm_w)), [DPAD_W, DPAD_NW, DPAD_SW], "◄"),
            (egui::Rect::from_center_size(
                pos2(dc.x + arm_h,   dc.y),
                vec2(arm_h, arm_w)), [DPAD_E, DPAD_NE, DPAD_SE], "►"),
        ];

        p.rect_filled(
            egui::Rect::from_center_size(dc, vec2(arm_w, arm_w)),
            CornerRadius::same(3),
            col_btn_off,
        );

        for (rect, dirs, label) in &dpad_rects {
            let active = dirs.contains(&dpad);
            p.rect_filled(*rect, cr, if active { col_dpad_active } else { col_btn_off });
            p.rect_stroke(*rect, cr,
                egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
            p.text(rect.center(), Align2::CENTER_CENTER, *label,
                   egui::FontId::proportional(10.0),
                   if active { Color32::from_rgb(20, 30, 50) } else { col_label });
        }  

        let fc    = pt(500.0, 152.0);
        let fb_r  = 16.0;
        let fb_d  = 34.0;

        struct FaceBtn {
            cx: f32, cy: f32,
            mask: u32,
            active_col: Color32,
            label: &'static str,
        }
        let face_btns = [
            FaceBtn { cx: fc.x,        cy: fc.y - fb_d, mask: BTN_TRIANGLE,
                      active_col: col_triangle, label: "△" },
            FaceBtn { cx: fc.x + fb_d, cy: fc.y,        mask: BTN_CIRCLE,
                      active_col: col_circle,   label: "○" },
            FaceBtn { cx: fc.x,        cy: fc.y + fb_d, mask: BTN_CROSS,
                      active_col: col_cross,    label: "✕" },
            FaceBtn { cx: fc.x - fb_d, cy: fc.y,        mask: BTN_SQUARE,
                      active_col: col_square,   label: "□" },
        ];

        for btn in &face_btns {
            let centre = pos2(px(btn.cx - o.x), py(btn.cy - o.y));
            let active = buttons & btn.mask != 0;
            p.circle_filled(centre, fb_r,
                if active { btn.active_col }
                else      { col_btn_off   });
            p.circle_stroke(centre, fb_r,
                egui::Stroke::new(1.0, col_btn_edge));
            p.text(centre, Align2::CENTER_CENTER, btn.label,
                   egui::FontId::proportional(13.0),
                   if active { Color32::WHITE } else { col_label });
        }

        let tp_rect = egui::Rect::from_min_max(pt(268.0, 74.0), pt(432.0, 182.0));
        let tp_pressed = buttons & BTN_TOUCHPAD != 0;

        p.rect_filled(tp_rect, CornerRadius::same(10),
            if tp_pressed { Color32::from_rgb(45, 65, 100) } else { Color32::from_rgb(22, 32, 50) });
        p.rect_stroke(tp_rect, CornerRadius::same(10),
            egui::Stroke::new(if tp_pressed { 1.5 } else { 1.0 },
                if tp_pressed { col_accent } else { col_body_edge }),
            egui::StrokeKind::Outside);

        if let Some(pts) = touch_pts {
            for tp in pts.iter().filter(|t| t.active) {
                let tx = tp_rect.min.x + (tp.x as f32 / TOUCHPAD_MAX_X as f32) * tp_rect.width();
                let ty = tp_rect.min.y + (tp.y as f32 / TOUCHPAD_MAX_Y as f32) * tp_rect.height();
                p.circle_filled(pos2(tx, ty), 7.0, col_accent);
                p.circle_stroke(pos2(tx, ty), 7.0,
                    egui::Stroke::new(1.0, Color32::WHITE));
            }
        }

        if touch_count == 0 {
            p.text(tp_rect.center(), Align2::CENTER_CENTER, "TOUCHPAD",
                   egui::FontId::proportional(10.0), col_label);
        }

        let create_pressed = buttons & BTN_CREATE != 0;
        let create_rect = egui::Rect::from_min_max(pt(236.0, 130.0), pt(264.0, 148.0));
        p.rect_filled(create_rect, CornerRadius::same(5),
            if create_pressed { col_system_active } else { col_btn_off });
        p.rect_stroke(create_rect, CornerRadius::same(5),
            egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
        p.text(create_rect.center(), Align2::CENTER_CENTER, "≡+",
               egui::FontId::proportional(9.0), col_label);

        let options_pressed = buttons & BTN_OPTIONS != 0;
        let opts_rect = egui::Rect::from_min_max(pt(436.0, 130.0), pt(464.0, 148.0));
        p.rect_filled(opts_rect, CornerRadius::same(5),
            if options_pressed { col_system_active } else { col_btn_off });
        p.rect_stroke(opts_rect, CornerRadius::same(5),
            egui::Stroke::new(1.0, col_btn_edge), egui::StrokeKind::Outside);
        p.text(opts_rect.center(), Align2::CENTER_CENTER, "≡",
               egui::FontId::proportional(9.0), col_label);

        let mute_pressed = buttons & BTN_MUTE != 0;
        let mute_c = pt(350.0, 66.0);
        p.circle_filled(mute_c, 10.0,
            if mute_pressed { col_system_active } else { col_btn_off });
        p.circle_stroke(mute_c, 10.0, egui::Stroke::new(1.0, col_btn_edge));
        p.text(mute_c, Align2::CENTER_CENTER, "🔇",
               egui::FontId::proportional(8.0), col_label);

        let ps_pressed = buttons & BTN_PS != 0;
        let ps_c = pt(350.0, 210.0);
        let ps_col = if ps_pressed {
            Color32::from_rgb(255, 255, 255)
        } else {
            col_btn_off
        };
        p.circle_filled(ps_c, 16.0, ps_col);
        p.circle_stroke(ps_c, 16.0, egui::Stroke::new(1.5, col_btn_edge));
        p.text(ps_c, Align2::CENTER_CENTER, "PS",
               egui::FontId::proportional(9.0),
               if ps_pressed { Color32::from_rgb(20, 30, 50) } else { col_label });

        Self::render_live_stick(
            &p, pt(150.0, 270.0), 42.0,
            [lx, ly], buttons & BTN_L3 != 0, [col_accent, col_btn_off, col_btn_edge],
        );

        Self::render_live_stick(
            &p, pt(440.0, 270.0), 42.0,
            [rx_ax, ry_ax], buttons & BTN_R3 != 0, [col_accent, col_btn_off, col_btn_edge],
        );

        p.text(pt(150.0, 320.0), Align2::CENTER_CENTER, "L3",
               egui::FontId::proportional(10.0), col_label);
        p.text(pt(440.0, 320.0), Align2::CENTER_CENTER, "R3",
               egui::FontId::proportional(10.0), col_label);

        ui.add_space(12.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!(
                "L2 {:3}   R2 {:3}   LX {:3}  LY {:3}   RX {:3}  RY {:3}   Touches {}",
                l2_raw, r2_raw, lx, ly, rx_ax, ry_ax, touch_count
            )).size(12.0).color(Color32::from_gray(120)).monospace());
        });
    }

    fn render_live_stick(
        p: &Painter,
        center: Pos2,
        radius: f32,
        raw: [u8; 2],
        pressed: bool,
        colors: [Color32; 3]
    ) {
        p.circle_filled(center, radius, colors[1]);
        p.circle_stroke(center, radius,
            egui::Stroke::new(if pressed { 2.5 } else { 1.5 },
                if pressed { colors[0] } else { colors[2] }));

        p.circle_stroke(center, radius * 0.55,
            egui::Stroke::new(0.5, Color32::from_rgb(40, 55, 80)));
        
        let nx = (raw[0] as f32 - 128.0) / 128.0;
        let ny = (raw[1] as f32 - 128.0) / 128.0;
        let dot = pos2(
            center.x + nx * (radius - 10.0),
            center.y + ny * (radius - 10.0),
        );
        p.circle_filled(dot, 8.0, colors[0]);
        p.circle_stroke(dot, 8.0, egui::Stroke::new(1.0, Color32::WHITE));
    }

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
        if self.controller.is_none() {
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
                    while let Ok(state) = rx.try_recv() {
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

