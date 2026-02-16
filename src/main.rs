use std::{sync::{mpsc::Receiver, Arc, Mutex}, time::{Duration, Instant}};

use eframe::App;
use egui::{Align, CentralPanel, Color32, Context, CornerRadius, Frame, Layout, Margin, RichText, ScrollArea, Ui};
use hidapi::HidApi;

use crate::{dualsense::{BatteryInfo, DualSense, MicLedState}, firmware::FirmwareDownloader};

mod dualsense;
mod firmware;

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 700.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "DS4U",
        options,
        Box::new(|_cc| Ok(Box::new(DS4UApp::new())))
    )
}

#[derive(Debug, Clone)]
enum ProgressUpdate {
    Progress(u32),
    Status(String),
    Complete,
    Error(String)
}

#[derive(PartialEq, Clone, Copy)]
enum TriggerMode {
    Off,
    Feedback
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

struct DS4UApp {
    api: HidApi,
    controller: Option<Arc<Mutex<DualSense>>>,
    devices: Vec<String>,
    selected_device: usize,

    lightbar: LightbarState,
    player_leds: u8,
    microphone: MicrophoneState,
    battery_info: Option<BatteryInfo>,
    last_battery_update: Instant,
    triggers: TriggerState,

    firmware_downloader: FirmwareDownloader,
    firmware_progress_rx: Option<Receiver<ProgressUpdate>>,
    firmware_progress: u32,
    firmware_status: String,
    firmware_updating: bool,

    status_message: String,
    error_message: String
}

impl DS4UApp {
    fn new() -> Self {
        let api = HidApi::new().unwrap();
        let devices = dualsense::list_devices(&api);

        Self {
            api,
            controller: None,
            devices,
            selected_device: 0,

            lightbar: LightbarState {
                r: 0.0, g: 0.5, b: 1.5, brightness: 255.0, enabled: true
            },

            player_leds: 1,

            microphone: MicrophoneState {
                enabled: false, led_state: MicLedState::Off
            },

            battery_info: None,
            last_battery_update: Instant::now() - Duration::from_secs(10),

            triggers: TriggerState {
                mode: TriggerMode::Off, position: 0, strength: 5
            },

            firmware_downloader: FirmwareDownloader::new(),
            firmware_progress_rx: None,
            firmware_progress: 0,
            firmware_status: String::new(),
            firmware_updating: false,

            status_message: String::new(),
            error_message: String::new()
        }
    }

    fn refresh_devices(&mut self) {
        self.devices = dualsense::list_devices(&self.api);
        if self.selected_device >= self.devices.len() {
            self.selected_device = 0;
        }
    }

    fn connect_controller(&mut self) {
        match DualSense::new(&self.api, None) {
            Ok(ds) => {
                self.controller = Some(Arc::new(Mutex::new(ds)));
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
                match ctrl.get_battery() {
                    Ok(info) => {
                        self.battery_info = Some(info);
                        self.last_battery_update = Instant::now();
                    }
                    Err(e) => {
                        self.error_message = format!("Battery read error: {}", e);
                    }
                }
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
                    }
                    ProgressUpdate::Error(e) => {
                        self.firmware_updating = false;
                        self.error_message = e;
                        self.firmware_progress = 0;
                    }
                }
            }
        }
    }

    fn render_header(&self, ui: &mut Ui) {
        ui.add_space(10.0);
        ui.heading(RichText::new("DS4U")
            .size(28.0)
            .color(Color32::WHITE));
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);
    }

    fn render_battery_panel(&self, ui: &mut Ui) {
        if let Some(battery) = &self.battery_info {
            Frame::NONE
                .fill(Color32::from_rgb(25, 25, 35))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(Margin::same(15))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Battery").size(18.0).strong());

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.label(&battery.status);
                            ui.label(RichText::new(format!("{}%", battery.capacity))
                                .size(24.0)
                                .color(
                                    if battery.capacity > 50 {
                                        Color32::from_rgb(0, 255, 100)
                                    } else if battery.capacity > 20 {
                                        Color32::from_rgb(255, 200, 0)
                                    } else {
                                        Color32::from_rgb(255, 50, 50)
                                    })
                                );
                        });
                    });
                });
        }
    }

    fn render_status_bar(&self, ui: &mut Ui) {
        ui.add_space(10.0);

        if !self.error_message.is_empty() {
            ui.colored_label(Color32::from_rgb(255, 100, 100), &self.error_message);
        }

        if !self.status_message.is_empty() { 
            ui.colored_label(Color32::from_rgb(100, 255, 100), &self.status_message);
        }
    }
}

impl App for DS4UApp {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        if self.controller.is_some() && 
            self.last_battery_update.elapsed() > Duration::from_secs(5) {
            self.update_battery();
        }

        self.check_firmware_progress();

        apply_style(ctx);

        CentralPanel::default().show(ctx, |ui| {
            self.render_header(ui);
            ui.add_space(15.0);

            self.render_battery_panel(ui);

            if self.battery_info.is_some() {
                ui.add_space(15.0);
            }

            if self.controller.is_some() {
                ScrollArea::vertical().show(ui, |ui| {
                    
                });
            }

            ui.with_layout(Layout::bottom_up(Align::LEFT), |ui| {
                self.render_status_bar(ui);
            });
        });
    }
}

fn apply_style(ctx: &Context) {
    let mut style = (*ctx.style()).clone();

    style.visuals.dark_mode = true;
    style.visuals.window_fill = Color32::from_rgb(15, 15, 20);
    style.visuals.panel_fill = Color32::from_rgb(20, 20, 28);
    style.visuals.extreme_bg_color = Color32::from_rgb(10, 10, 15);

    let accent_color = Color32::from_rgb(0, 122, 255);
    style.visuals.selection.bg_fill = accent_color;
    style.visuals.widgets.active.bg_fill = accent_color;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(50, 50, 70);
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(30, 30, 40);

    style.visuals.window_corner_radius = CornerRadius::same(12);
    style.visuals.widgets.noninteractive.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.inactive.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.hovered.corner_radius = CornerRadius::same(8);
    style.visuals.widgets.active.corner_radius = CornerRadius::same(8);

    ctx.set_style(style);
}

