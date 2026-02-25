use std::{
    sync::{self, Arc, Mutex, mpsc, mpsc::Receiver},
    thread::{self, sleep},
    time::{Duration, Instant},
};
use hidapi::HidApi;

use crate::{
    common::*, daemon::DaemonManager, dualsense::{self, BatteryInfo, DualSense}, firmware::FirmwareDownloader, inputs::ControllerState, ipc::{socket_path, IpcClient}, profiles::{Profile, ProfileManager}, settings::{Settings, SettingsManager}, state::*, theme::{Theme, ThemeManager}, transform::InputTransform
};

pub(crate) struct DS4UApp {
    pub(crate) settings: Settings,
    pub(crate) settings_manager: SettingsManager,
    pub(crate) theme: Theme,
    pub(crate) theme_manager: ThemeManager,

    api: HidApi,
    pub(crate) controller: Option<Arc<Mutex<DualSense>>>,

    pub(crate) ipc: Option<Arc<Mutex<IpcClient>>>,

    pub(crate) last_connection_check: Instant,

    pub(crate) active_section: Section,
    pub(crate) show_profiles_panel: bool,

    pub(crate) controller_is_bt: Option<bool>,
    pub(crate) controller_product_id: Option<u16>,

    profile_manager: ProfileManager,
    current_profile: Option<Profile>,
    profile_edit_name: String,

    daemon_manager: DaemonManager,

    pub(crate) battery_info: Option<BatteryInfo>,
    pub(crate) last_battery_update: Instant,
    
    pub(crate) lightbar: LightbarState,
    pub(crate) player_leds: u8,
    pub(crate) microphone: MicrophoneState,
    pub(crate) triggers: TriggerState,
    pub(crate) sticks: StickSettings,
    pub(crate) audio: AudioSettings,
    pub(crate) vibration: VibrationSettings,

    firmware_downloader: FirmwareDownloader,
    firmware_progress_rx: Option<Receiver<ProgressUpdate>>,
    pub(crate) firmware_progress: u32,
    pub(crate) firmware_status: String,
    
    pub(crate) firmware_updating: bool,
    fw_used_daemon: bool,

    update_mode_flag: Option<Arc<sync::atomic::AtomicBool>>,
    
    pub(crate) controller_serial: Option<String>,
    pub(crate) firmware_current_version: Option<u16>,
    pub(crate) firmware_latest_version: Option<String>,
    pub(crate) firmware_checking_latest: bool,
    pub(crate) firmware_build_date: Option<String>,
    pub(crate) firmware_build_time: Option<String>,

    pub(crate) status_message: String,
    pub(crate) error_message: String,

    pub(crate) controller_state: Option<ControllerState>,
    pub(crate) input_state_rx: Option<mpsc::Receiver<ControllerState>>,
    pub(crate) input_polling: bool,
    input_poll_stop: Option<Arc<sync::atomic::AtomicBool>>,

    pending_connect_since: Option<Instant>,
    pub(crate) input_transform: InputTransform
}

impl DS4UApp {
    pub(crate) fn new() -> Self {
        let api = HidApi::new().unwrap();

        let settings_manager = SettingsManager::new();
        let settings = settings_manager.load();
        let theme_manager = ThemeManager::new();
        let theme = theme_manager.load_by_id(&settings.theme_id);

        let mut app = Self {
            settings,
            settings_manager,
            theme,
            theme_manager,

            api,
            controller: None,
            ipc: None,
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
            fw_used_daemon: false,

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

            pending_connect_since: None,
            input_transform: InputTransform::default()
        };

        app.check_for_controller();
        app
    }
    
    pub(crate) fn is_connected(&self) -> bool {
        self.controller.is_some() || self.ipc.is_some()
    }

    pub(crate) fn start_input_polling(&mut self) {
        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(sync::atomic::AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        self.input_state_rx = Some(rx);
        self.input_poll_stop = Some(stop_flag);
        self.input_polling = true;

        if self.ipc.is_some() {
            let path = socket_path();
            thread::spawn(move || {
                let mut client = match IpcClient::connect(&path) {
                    Ok(c) => c,
                    Err(_) => return,
                };
                while !stop_clone.load(sync::atomic::Ordering::Relaxed) {
                    match client.get_input_state() {
                        Ok(state) => { let _ = tx.send(state); }
                        Err(_)    => { sleep(Duration::from_millis(8)); }
                    }
                }
            });
        } else {
            let Some(ctrl) = self.controller.clone() else { return };
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
    }

    pub(crate) fn stop_input_polling(&mut self) {
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

    fn connect_via_daemon(&mut self, client: Arc<Mutex<IpcClient>>) {
        let mut c = client.lock().unwrap();

        if let Ok(Some((serial, pid, is_bt))) = c.get_controller_info() {
            self.controller_serial = Some(serial);
            self.controller_is_bt = Some(is_bt);
            self.controller_product_id = Some(pid);
        } 

        if let Ok((ver, date, time)) = c.get_firmware_info() {
            self.firmware_current_version = Some(ver);
            self.firmware_build_date = Some(date);
            self.firmware_build_time = Some(time);
        }

        drop(c);

        self.firmware_latest_version = None;
        self.ipc = Some(client);
        self.status_message = "Controller connected (via daemon)".to_string();
        self.error_message.clear();
        self.lightbar.enabled = true;
        self.update_battery();
    }

    fn disconnect_controller(&mut self) {
        self.controller = None;
        self.battery_info = None;
        self.ipc = None;
        self.controller_is_bt = None;
        self.controller_product_id = None;
        self.controller_serial = None;
        self.status_message = "Controller disconnected".to_string();
    }

    pub(crate) fn update_battery(&mut self) {
        self.last_battery_update = Instant::now();

        if self.firmware_updating {
            return;
        }

        if let Some(ref ipc) = self.ipc.clone() {
            match ipc.lock().unwrap().get_battery() {
                Ok(info) => self.battery_info = Some(info),
                Err(_) => {
                    self.stop_input_polling();
                    self.disconnect_controller();
                }
            }
            return;
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

    pub(crate) fn check_firmware_progress(&mut self) {
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
                        self.daemon_manager.set_update_in_progress(false);
                        if self.fw_used_daemon {
                            self.fw_used_daemon = false;
                            self.controller = None;
                        }
                    }
                    ProgressUpdate::Error(e) => {
                        self.firmware_updating = false;
                        self.firmware_checking_latest = false;
                        self.error_message = e;
                        self.firmware_progress = 0;
                        self.update_mode_flag = None;
                        self.daemon_manager.set_update_in_progress(false);
                        if self.fw_used_daemon {
                            self.fw_used_daemon = false;
                            self.controller = None;
                        }
                    }
                    ProgressUpdate::LatestVersion(v) => {
                        self.firmware_latest_version = Some(v);
                        self.firmware_checking_latest = false;
                    }
                }
            }
        }
    }

    fn acquire_direct_fw(&mut self) -> bool {
        if self.controller.is_some() { return true; }

        self.daemon_manager.set_update_in_progress(true);
        sleep(Duration::from_millis(1500));

        if self.api.refresh_devices().is_err() {
            self.daemon_manager.set_update_in_progress(false);
            return false;
        }

        match DualSense::new(&self.api, None) {
            Ok(ds) => {
                self.controller = Some(Arc::new(Mutex::new(ds)));
                self.fw_used_daemon = true;
                true
            }
            Err(e) => {
                self.error_message = format!("Cannot open device for flash update: {}", e);
                self.daemon_manager.set_update_in_progress(false);
                false
            }
        }
    }

    pub(crate) fn apply_lightbar(&mut self) {
        let (r, g, b, br) = (
            (self.lightbar.r * 255.0) as u8,
            (self.lightbar.g * 255.0) as u8,
            (self.lightbar.b * 255.0) as u8,
            self.lightbar.brightness as u8
        );

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = ipc.lock().unwrap().set_lightbar(r, g, b, br);
            return;
        }

        if let Some(controller) = &self.controller 
            && let Ok(mut ctrl) = controller.lock() {
                let _ = ctrl.set_lightbar(r, g, b, br);
        }
    }

    pub(crate) fn apply_player_leds(&mut self) {
        let leds = self.player_leds;

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = ipc.lock().unwrap().set_player_leds(leds);
            return;
        }

        if let Some(controller) = &self.controller
            && let Ok(mut ctrl) = controller.lock()
        {
            let _ = ctrl.set_player_leds(leds);
        }
    }

    pub(crate) fn apply_microphone(&mut self) {
        let (enabled, led) = (self.microphone.enabled, self.microphone.led_state);
        
        if let Some(ref ipc) = self.ipc.clone() {
            let _ = ipc.lock().unwrap().set_mic(enabled);
            let _ = ipc.lock().unwrap().set_mic_led(led);
            return;
        }
        
        if let Some(controller) = &self.controller
            && let Ok(mut ctrl) = controller.lock()
        {
            let _ = ctrl.set_mic(enabled);
            let _ = ctrl.set_mic_led(led);
        }
    }

    pub(crate) fn apply_vibration(&mut self) {
        let (r, t) = (self.vibration.rumble, self.vibration.trigger);

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = ipc.lock().unwrap().set_vibration(r, t);
            return;
        }

        if let Some(controller) = &self.controller
            && let Ok(mut ctrl) = controller.lock()
        {
            let _ = ctrl.set_vibration(r, t);
        }
    }

    pub(crate) fn apply_trigger(&mut self) {
        match self.triggers.mode {
            TriggerMode::Off => {
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap().set_trigger_off();
                    return;
                }
                if let Some(c) = &self.controller && let Ok(mut ctrl) = c.lock() {
                    let _ = ctrl.set_trigger_off();
                }
            }
            TriggerMode::Feedback => {
                let mut strengths = [0u8; 10];
                for i in self.triggers.position..10 {
                    strengths[i as usize] = self.triggers.strength;
                }
                let mut active_zones: u16 = 0;
                let mut strength_zones: u32 = 0;
                for i in 0..10 {
                    if strengths[i] > 0 {
                        let sv = ((strengths[i] - 1) & 0x07) as u32;
                        strength_zones |= sv << (3 * i);
                        active_zones |= 1 << i;
                    }
                }
                let params: [u8; 10] = [
                    (active_zones & 0xff) as u8,
                    ((active_zones >> 8) & 0xff) as u8,
                    (strength_zones & 0xff) as u8,
                    ((strength_zones >> 8) & 0xff) as u8,
                    ((strength_zones >> 16) & 0xff) as u8,
                    ((strength_zones >> 24) & 0xff) as u8,
                    0, 0, 0, 0,
                ];
                if let Some(ref ipc) = self.ipc.clone() {
                    let _ = ipc.lock().unwrap()
                        .set_trigger_effect(true, true, 0x21, params);
                    return;
                }
                if let Some(c) = &self.controller && let Ok(mut ctrl) = c.lock() {
                    let _ = ctrl.set_trigger_effect(true, true, 0x21, &params);
                }
            }
            _ => {}
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

    pub(crate) fn check_controller_connection(&mut self) {
        if self.firmware_updating {
            return;
        }

        if let Some(ref ipc) = self.ipc.clone() {
            let still_present = matches!(
                ipc.lock().unwrap().get_controller_info(),
                Ok(Some(_))
            );
            if !still_present {
                self.stop_input_polling();
                self.disconnect_controller();
            }
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

    pub(crate) fn check_for_controller(&mut self) {
        self.last_connection_check = Instant::now();

        if self.daemon_manager.is_active() 
            && self.ipc.is_none()
            && let Some(client) = self.daemon_manager.connect_new_client()
        {
            let available = matches!(
                client.lock().unwrap().get_controller_info(),
                Ok(Some(_))
            );
            if available {
                self.connect_via_daemon(client);
            }
            return;
        }

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


    pub(crate) fn fetch_latest_verision_async(&mut self) {
        if self.firmware_checking_latest {
            return;
        }

        let Some(pid) = self.controller_product_id else { return };
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

    pub(crate) fn flash_latest(&mut self) {
        self.stop_input_polling();

        if !self.acquire_direct_fw() { return; }

        let pid = self.controller_product_id.unwrap_or(DS_PID);
        let ctrl = Arc::clone(self.controller.as_ref().unwrap());
        let (tx, rx) = mpsc::channel();
        let downloader = self.firmware_downloader.clone();
        
        self.firmware_progress_rx = Some(rx);
        self.firmware_updating = true;
        self.firmware_progress = 0;
        self.firmware_status = "Downloading latest firmware...".to_string();

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

    pub(crate) fn flash_file(&mut self) {
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

        if !self.acquire_direct_fw() { return; }

        let ctrl = Arc::clone(self.controller.as_ref().unwrap());
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

    pub(crate) fn apply_input_transform(&mut self) {
        let mut t = self.current_profile
            .as_ref()
            .map(|p| p.to_input_transform())
            .unwrap_or_default();
        t.left_curve     = self.sticks.left_curve.clone();
        t.right_curve    = self.sticks.right_curve.clone();
        t.left_deadzone  = self.sticks.left_deadzone;
        t.right_deadzone = self.sticks.right_deadzone;

        self.input_transform = t.clone();

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = ipc.lock().unwrap().set_input_transform(t);
        }
    }
}
