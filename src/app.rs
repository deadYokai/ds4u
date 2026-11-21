use hidapi::HidApi;
use std::{
    sync::{self, Arc, Mutex, mpsc},
    thread::{self, sleep},
    time::{Duration, Instant},
};

use crate::{
    backend::{ControllerBackend, DirectBackend, IpcBackend, TRIGGER_OFF},
    common::*,
    daemon::DaemonManager,
    dualsense::{self, BatteryInfo, DeviceInfo, DualSense},
    firmware_controller::FirmwareController,
    input_poller::InputPoller,
    ipc::{IpcClient, socket_path},
    profiles::{Profile, ProfileManager, TriggerConfig},
    settings::{Settings, SettingsManager},
    state::*,
    theme::{Theme, ThemeManager},
    transform::{GyroProcessor, InputTransform},
    util::mlock,
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

    pub(crate) controller_is_bt: Option<bool>,
    pub(crate) controller_product_id: Option<u16>,

    pub(crate) profile_manager: ProfileManager,
    pub(crate) current_profile: Option<Profile>,
    pub(crate) profile_edit_name: String,

    daemon_manager: DaemonManager,

    pub(crate) battery_info: Option<BatteryInfo>,
    pub(crate) last_battery_update: Instant,

    pub(crate) lightbar: LightbarState,
    pub(crate) player_leds: u8,
    pub(crate) microphone: MicrophoneState,
    pub(crate) triggers: TriggersState,
    pub(crate) sticks: StickSettings,
    pub(crate) audio: AudioSettings,
    pub(crate) vibration: VibrationSettings,
    pub(crate) gyro: GyroState,
    pub(crate) touchpad: TouchpadState,
    pub(crate) haptic_state: HapticState,

    pub(crate) firmware: FirmwareController,
    pub(crate) input: InputPoller,
    pub(crate) nav: crate::ui::navigation::Navigator,
    pub(crate) egui_ctx: Option<egui::Context>,

    pub(crate) controller_serial: Option<String>,
    pub(crate) devices: Vec<DeviceInfo>,
    pub(crate) selected_device: Option<String>,

    pub(crate) status_message: String,
    pub(crate) error_message: String,

    pending_connect_since: Option<Instant>,
    pub(crate) input_transform: InputTransform,

    pub(crate) lightbar_effect: LightbarEffect,
    pub(crate) local_gyro: GyroProcessor,

    pub(crate) daemon_alive_cached: bool,
    last_daemon_probe: Instant,
    haptic_stream: crate::haptics_stream::HapticStream,
    usb_haptic_stream: crate::usb_haptics::UsbHapticStream,
    daemon_raw_active: bool,
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
                enabled: true,
            },

            player_leds: 1,

            microphone: MicrophoneState {
                enabled: false,
                led_state: MicLedState::Off,
            },

            triggers: TriggersState {
                left: TriggerConfig::default(),
                right: TriggerConfig::default(),
            },

            sticks: StickSettings {
                left_curve: SensitivityCurve::Default,
                right_curve: SensitivityCurve::Default,
                left_deadzone: 0.1,
                right_deadzone: 0.1,
                left_outer_deadzone: 1.0,
                right_outer_deadzone: 1.0,
                left_invert_x: false,
                left_invert_y: false,
                right_invert_x: false,
                right_invert_y: false,
                swap: false,
            },

            audio: AudioSettings {
                volume: 0,
                speaker_mode: SpeakerMode::Internal,
            },

            vibration: VibrationSettings {
                rumble: 0,
                trigger: 0,
            },

            gyro: GyroState {
                processor: GyroProcessor::default(),
            },

            touchpad: TouchpadState {
                show_overlay: true,
                mode: TouchpadMode::Mouse,
                tap_to_click: true,
                natural_scrolling: false,
                sensitivity: 1.0,
            },

            haptic_state: HapticState {
                pattern: HapticPattern::None,
                strength: 0,
                speed: 1.0,
            },

            firmware: FirmwareController::new(),
            input: InputPoller::new(),
            nav: crate::ui::navigation::Navigator::new(),
            egui_ctx: None,

            controller_serial: None,
            devices: Vec::new(),
            selected_device: None,

            status_message: String::new(),
            error_message: String::new(),

            pending_connect_since: None,
            input_transform: InputTransform::default(),

            lightbar_effect: LightbarEffect::None,

            local_gyro: GyroProcessor::default(),

            daemon_alive_cached: false,
            last_daemon_probe: Instant::now() - Duration::from_secs(10),
            haptic_stream: crate::haptics_stream::HapticStream::new(),
            usb_haptic_stream: crate::usb_haptics::UsbHapticStream::new(),
            daemon_raw_active: false,
        };

        app.check_for_controller();

        if app.current_profile.is_none() {
            app.load_device_profile();
        }

        app
    }

    pub(crate) fn is_connected(&self) -> bool {
        self.controller.is_some() || self.ipc.is_some()
    }

    fn backend(&self) -> Option<Box<dyn ControllerBackend>> {
        if let Some(ipc) = &self.ipc {
            return Some(Box::new(IpcBackend(ipc.clone())));
        }
        if let Some(ctrl) = &self.controller {
            return Some(Box::new(DirectBackend(ctrl.clone())));
        }
        None
    }

    pub(crate) fn start_input_polling(&mut self) {
        let (tx, rx) = mpsc::channel();
        let stop_flag = Arc::new(sync::atomic::AtomicBool::new(false));
        let stop_clone = Arc::clone(&stop_flag);

        self.input.state_rx = Some(rx);
        self.input.stop = Some(stop_flag);
        self.input.polling = true;

        let waker = self.egui_ctx.clone();

        let handle = if self.ipc.is_some() {
            let path = socket_path();
            let sel = self.selected_device.clone();
            thread::spawn(move || {
                let mut client = match IpcClient::connect(&path) {
                    Ok(c) => c,
                    Err(_) => return,
                };

                if let Some(id) = sel.as_deref()
                    && client.select_device(Some(id)).is_err()
                {
                    return;
                }

                let mut last_b = u32::MAX;
                let mut last_d = 0xFFu8;
                while !stop_clone.load(sync::atomic::Ordering::Relaxed) {
                    match client.get_input_state() {
                        Ok(state) => {
                            let (b, d, ry) = (state.buttons, state.dpad, state.right_y);
                            let _ = tx.send(state);
                            if let Some(ctx) = &waker {
                                let stick = (ry as i16 - 128).abs() > 24;
                                if b != last_b || d != last_d || stick {
                                    ctx.request_repaint();
                                }
                            }
                            last_b = b;
                            last_d = d;
                            sleep(Duration::from_millis(8));
                        }
                        Err(_) => {
                            sleep(Duration::from_millis(8));
                        }
                    }
                }
            })
        } else {
            if self.controller.is_none() {
                return;
            }
            let serial = self.controller_serial.clone();
            thread::spawn(move || {
                let api = match HidApi::new() {
                    Ok(a) => a,
                    Err(_) => return,
                };
                let mut reader = match DualSense::new(&api, serial.as_deref()) {
                    Ok(d) => d,
                    Err(_) => return,
                };
                let mut last_b = u32::MAX;
                let mut last_d = 0xFFu8;
                while !stop_clone.load(sync::atomic::Ordering::Relaxed) {
                    match reader.get_input_state() {
                        Ok(state) => {
                            let (b, d, ry) = (state.buttons, state.dpad, state.right_y);
                            let _ = tx.send(state);
                            if let Some(ctx) = &waker {
                                let stick = (ry as i16 - 128).abs() > 24;
                                if b != last_b || d != last_d || stick {
                                    ctx.request_repaint();
                                }
                            }
                            last_b = b;
                            last_d = d;
                        }
                        Err(_) => {
                            sleep(Duration::from_millis(8));
                        }
                    }
                }
            })
        };
        self.input.thread = Some(handle);
    }

    pub(crate) fn stop_input_polling(&mut self) {
        self.input.stop();
    }

    fn connect_controller(&mut self) {
        let opened = DualSense::new(&self.api, self.selected_device.as_deref())
            .or_else(|_| DualSense::new(&self.api, None));
        match opened {
            Ok(ds) => {
                self.selected_device = Some(ds.serial().to_string());
                if let Ok((version, build_date, build_time)) = ds.get_firmware_info() {
                    self.firmware.current_version = Some(version);
                    self.firmware.build_date = Some(build_date);
                    self.firmware.build_time = Some(build_time);
                } else {
                    self.firmware.current_version = None;
                    self.firmware.build_date = None;
                    self.firmware.build_time = None;
                }

                self.controller_serial = Some(ds.serial().to_string());
                self.controller_is_bt = Some(ds.is_bluetooth());
                self.controller_product_id = Some(ds.product_id());
                self.controller = Some(Arc::new(Mutex::new(ds)));
                self.firmware.latest_version = None;
                self.status_message = "Controller connected".to_string();
                self.error_message.clear();
                self.lightbar.enabled = true;
                self.update_battery();
                self.apply_lightbar();
                self.apply_player_leds();
                self.apply_microphone();
                self.apply_input_transform();
                self.apply_triggers();
            }
            Err(_) => {
                self.controller = None;
            }
        }
    }

    pub(crate) fn connect_via_daemon(&mut self, client: Arc<Mutex<IpcClient>>) {
        let mut c = mlock(&client);

        if let Ok(list) = c.list_devices() {
            self.devices = list;
        }
        let pick = self
            .selected_device
            .clone()
            .filter(|id| self.devices.iter().any(|d| &d.id == id))
            .or_else(|| self.devices.first().map(|d| d.id.clone()));

        let Some(id) = pick else {
            drop(c);
            self.ipc = Some(client);
            self.status_message = "Attached to daemon (waiting for device)".into();
            return;
        };
        if c.select_device(Some(&id)).is_err() {
            return;
        }
        self.selected_device = Some(id);

        if let Ok(Some((serial, pid, is_bt))) = c.get_controller_info() {
            self.controller_serial = Some(serial);
            self.controller_is_bt = Some(is_bt);
            self.controller_product_id = Some(pid);
        }
        if let Ok((ver, date, time)) = c.get_firmware_info() {
            self.firmware.current_version = Some(ver);
            self.firmware.build_date = Some(date);
            self.firmware.build_time = Some(time);
        }
        drop(c);

        self.firmware.latest_version = None;
        self.ipc = Some(client);
        self.status_message = "Controller connected (via daemon)".to_string();
        self.error_message.clear();
        self.lightbar.enabled = true;

        self.load_device_profile();
        self.update_battery();
    }

    pub(crate) fn load_device_profile(&mut self) {
        let name = self
            .controller_serial
            .as_deref()
            .map(|id| self.settings.profile_for(id).to_string())
            .unwrap_or_else(|| self.settings.profile.clone());
        let name = if name.is_empty() {
            "Default".to_string()
        } else {
            name
        };

        let profile = if self.profile_manager.profile_exists(&name) {
            self.profile_manager
                .load_profile(&name)
                .unwrap_or_else(|_| self.profile_manager.ensure_default_exists())
        } else {
            self.profile_manager.ensure_default_exists()
        };
        self.load_profile(&profile);
    }

    pub(crate) fn switch_device(&mut self, id: &str) {
        if self.selected_device.as_deref() == Some(id) && self.is_connected() {
            return;
        }
        self.stop_input_polling();
        if self.haptic_stream_active() {
            self.stop_raw_haptics();
        }
        self.input.controller_state = None;
        self.battery_info = None;
        self.selected_device = Some(id.to_string());

        if let Some(client) = self.ipc.clone() {
            self.connect_via_daemon(client);
        } else {
            self.controller = None;
            self.connect_controller();
            self.load_device_profile();
        }
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

        if self.firmware.updating {
            return;
        }

        if let Some(ref ipc) = self.ipc.clone() {
            match mlock(ipc).get_battery() {
                Ok(info) => self.battery_info = Some(info),
                Err(_) => {
                    self.stop_input_polling();
                    self.disconnect_controller();
                }
            }
            return;
        }

        let Some(controller) = &self.controller else {
            return;
        };
        let Ok(mut ctrl) = controller.try_lock() else {
            return;
        };

        if let Ok(info) = ctrl.get_battery() {
            self.battery_info = Some(info);
        } else {
            drop(ctrl);
            self.disconnect_controller();
        }
    }

    pub(crate) fn check_firmware_progress(&mut self) {
        let updates: Vec<ProgressUpdate> = self
            .firmware
            .progress_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        for update in updates {
            match update {
                ProgressUpdate::Progress(p) => {
                    self.firmware.progress = p;
                }
                ProgressUpdate::Status(s) => {
                    self.firmware.status = s;
                }
                ProgressUpdate::Complete => {
                    self.firmware.updating = false;
                    self.firmware.last_flash_result = Some(Ok(()));
                    self.status_message = "Firmware update completed".to_string();
                    self.firmware.progress = 100;
                    self.firmware.update_mode_flag = None;
                    self.daemon_manager
                        .set_update_in_progress(self.selected_device.as_deref(), false);
                    if self.firmware.used_daemon {
                        self.firmware.used_daemon = false;
                        self.controller = None;
                    }
                    self.firmware.reap_thread();
                }
                ProgressUpdate::Error(e) => {
                    let was_flashing = self.firmware.updating;
                    self.firmware.updating = false;
                    self.firmware.checking_latest = false;
                    if was_flashing {
                        self.firmware.last_flash_result = Some(Err(e.clone()));
                    }
                    self.error_message = e;
                    self.firmware.progress = 0;
                    self.firmware.update_mode_flag = None;
                    self.daemon_manager
                        .set_update_in_progress(self.selected_device.as_deref(), false);
                    if self.firmware.used_daemon {
                        self.firmware.used_daemon = false;
                        self.controller = None;
                    }
                    self.firmware.reap_thread();
                }
                ProgressUpdate::LatestVersion(v) => {
                    self.firmware.latest_version = Some(v);
                    self.firmware.checking_latest = false;
                    self.firmware.reap_thread();
                }
            }
        }
    }

    pub(crate) fn haptic_stream_active(&self) -> bool {
        self.haptic_stream.is_active()
            || self.usb_haptic_stream.is_active()
            || self.daemon_raw_active
    }

    pub(crate) fn start_raw_haptics(&mut self) {
        let pattern = self.haptic_state.pattern;
        let strength = self.haptic_state.strength;
        let speed = self.haptic_state.speed;
        self.error_message.clear();
        match self.controller_is_bt {
            Some(true) => {
                if let Some(ctrl) = self.controller.clone() {
                    self.stop_input_polling();
                    self.haptic_stream.start(ctrl, pattern, strength, speed);
                    self.status_message = "Streaming raw haptics (Bluetooth)".to_string();
                } else if let Some(ipc) = self.ipc.clone() {
                    match mlock(&ipc).set_raw_haptics(true) {
                        Ok(()) => {
                            self.daemon_raw_active = true;
                            self.status_message =
                                "Streaming raw haptics (Bluetooth via daemon)".to_string();
                        }
                        Err(e) => self.error_message = format!("Daemon haptics failed: {e}"),
                    }
                } else {
                    self.error_message = "No controller connected".to_string();
                }
            }
            Some(false) => {
                let Some(card) = crate::usb_haptics::find_dualsense_card() else {
                    self.error_message =
                        "Couldn't find the controller's USB audio device".to_string();
                    return;
                };
                let _ = self.usb_haptic_stream.start(card, pattern, strength, speed);
                self.status_message = "Streaming raw haptics (USB)".to_string();
            }
            None => {
                self.error_message = "No controller connected".to_string();
            }
        }
    }

    pub(crate) fn stop_raw_haptics(&mut self) {
        self.haptic_stream.stop();
        self.usb_haptic_stream.stop();
        if self.daemon_raw_active {
            if let Some(ipc) = self.ipc.clone() {
                let _ = mlock(&ipc).set_raw_haptics(false);
            }
            self.daemon_raw_active = false;
        }
        self.status_message = "Raw haptics stopped".to_string();
    }

    pub(crate) fn update_raw_haptics(&mut self) {
        let p = self.haptic_state.pattern;
        let s = self.haptic_state.strength;
        let sp = self.haptic_state.speed;
        self.haptic_stream.set_params(p, s, sp);
        self.usb_haptic_stream.set_params(p, s, sp);
    }

    fn acquire_direct_fw(&mut self) -> bool {
        if self.controller.is_some() {
            return true;
        }

        self.daemon_manager
            .set_update_in_progress(self.selected_device.as_deref(), true);
        sleep(Duration::from_millis(1500));

        if self.api.refresh_devices().is_err() {
            self.daemon_manager
                .set_update_in_progress(self.selected_device.as_deref(), false);
            return false;
        }

        match DualSense::new(&self.api, self.selected_device.as_deref()) {
            Ok(ds) => {
                self.controller = Some(Arc::new(Mutex::new(ds)));
                self.firmware.used_daemon = true;
                true
            }
            Err(e) => {
                self.error_message = format!("Cannot open device for flash update: {}", e);
                self.daemon_manager
                    .set_update_in_progress(self.selected_device.as_deref(), false);
                false
            }
        }
    }

    pub(crate) fn apply_lightbar(&mut self) {
        let r = (self.lightbar.r * 255.0) as u8;
        let g = (self.lightbar.g * 255.0) as u8;
        let b = (self.lightbar.b * 255.0) as u8;
        let br = self.lightbar.brightness as u8;
        if let Some(be) = self.backend() {
            be.set_lightbar(r, g, b, br);
        }
    }

    pub(crate) fn apply_lightbar_effect(&mut self) {
        if let Some(be) = self.backend() {
            be.set_lightbar_effect(self.lightbar_effect.clone());
        }
    }

    pub(crate) fn apply_haptic_pattern(&mut self) {
        if let Some(be) = self.backend() {
            be.set_haptic_pattern(
                self.haptic_state.pattern,
                self.haptic_state.strength,
                self.haptic_state.speed,
            );
        }
    }

    pub(crate) fn apply_gyro(&mut self) {
        let g = &self.gyro.processor;
        self.local_gyro.enabled = g.enabled;
        self.local_gyro.smoothing = g.smoothing;
        self.local_gyro.sensitivity = g.sensitivity;

        if let Some(be) = self.backend() {
            be.set_gyro(g.enabled, g.smoothing, g.sensitivity);
        }
    }

    pub(crate) fn apply_player_leds(&mut self) {
        if let Some(be) = self.backend() {
            be.set_player_leds(self.player_leds);
        }
    }

    pub(crate) fn apply_microphone(&mut self) {
        if let Some(be) = self.backend() {
            be.set_mic(self.microphone.enabled);
            be.set_mic_led(self.microphone.led_state);
        }
    }

    pub(crate) fn apply_vibration(&mut self) {
        if let Some(be) = self.backend() {
            be.set_vibration(self.vibration.rumble, self.vibration.trigger);
        }
    }

    pub(crate) fn test_rumble(&self, amp_l: u8, amp_r: u8, duration_ms: u64) {
        let ctrl = self.controller.clone();
        let ipc = self.ipc.clone();
        std::thread::spawn(move || {
            if let Some(ctrl) = &ctrl
                && let Ok(mut c) = ctrl.lock()
            {
                let _ = c.set_rumble(amp_l, amp_r);
            } else if let Some(ipc) = &ipc
                && let Ok(mut c) = ipc.lock()
            {
                let _ = c.set_rumble(amp_l, amp_r);
            }
            std::thread::sleep(std::time::Duration::from_millis(duration_ms));
            if let Some(ctrl) = &ctrl
                && let Ok(mut c) = ctrl.lock()
            {
                let _ = c.set_rumble(0, 0);
            } else if let Some(ipc) = &ipc
                && let Ok(mut c) = ipc.lock()
            {
                let _ = c.set_rumble(0, 0);
            }
        });
    }

    pub(crate) fn apply_triggers(&mut self) {
        let to_eff = |cfg: &TriggerConfig| {
            if matches!(cfg.mode, TriggerMode::Off) {
                None
            } else {
                Some(cfg.to_effect())
            }
        };
        let l = to_eff(&self.triggers.left).or(Some(TRIGGER_OFF));
        let r = to_eff(&self.triggers.right).or(Some(TRIGGER_OFF));
        if let Some(be) = self.backend() {
            be.set_trigger_effects(l, r);
        }
    }

    pub(crate) fn load_profile(&mut self, profile: &Profile) {
        self.lightbar.r = profile.lightbar_r;
        self.lightbar.g = profile.lightbar_g;
        self.lightbar.b = profile.lightbar_b;
        self.lightbar.brightness = profile.lightbar_brightness;
        self.player_leds = profile.player_leds;
        self.microphone.enabled = profile.mic_enabled;

        self.sticks.left_curve = profile.stick_left_curve.clone();
        self.sticks.right_curve = profile.stick_right_curve.clone();
        self.sticks.left_deadzone = profile.stick_left_deadzone;
        self.sticks.right_deadzone = profile.stick_right_deadzone;
        self.sticks.left_outer_deadzone = profile.stick_left_outer_deadzone;
        self.sticks.right_outer_deadzone = profile.stick_right_outer_deadzone;
        self.sticks.left_invert_x = profile.stick_left_invert_x;
        self.sticks.left_invert_y = profile.stick_left_invert_y;
        self.sticks.right_invert_x = profile.stick_right_invert_x;
        self.sticks.right_invert_y = profile.stick_right_invert_y;
        self.sticks.swap = profile.stick_swap;

        self.triggers.left = profile.trigger_left_config.clone();
        self.triggers.right = profile.trigger_right_config.clone();

        self.gyro.processor = profile.to_gyro_processor();
        self.touchpad.mode = if !profile.touchpad_enabled {
            TouchpadMode::Disabled
        } else {
            profile.touchpad_mode
        };
        self.touchpad.show_overlay = profile.touchpad_show_overlay;
        self.touchpad.tap_to_click = profile.touchpad_tap_to_click;
        self.touchpad.natural_scrolling = profile.touchpad_natural_scrolling;
        self.touchpad.sensitivity = profile.touchpad_sensitivity;

        self.haptic_state.pattern = profile.haptic_pattern;
        self.haptic_state.strength = profile.haptic_strength;
        self.haptic_state.speed = profile.haptic_speed;

        self.current_profile = Some(profile.clone());

        match self.controller_serial.clone() {
            Some(id) => self.settings.set_profile_for(&id, &profile.name),
            None => self.settings.profile = profile.name.clone(),
        }
        self.settings_manager.save(&self.settings);

        self.apply_lightbar();
        self.apply_player_leds();
        self.apply_microphone();
        self.apply_input_transform();
        self.apply_triggers();
        self.apply_gyro();
        self.apply_haptic_pattern();

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = mlock(ipc).switch_profile(&profile.name);
        }
    }

    pub(crate) fn sync_profile(&mut self) {
        let Some(profile) = self.current_profile.as_mut() else {
            return;
        };

        profile.lightbar_r = self.lightbar.r;
        profile.lightbar_g = self.lightbar.g;
        profile.lightbar_b = self.lightbar.b;
        profile.lightbar_brightness = self.lightbar.brightness;
        profile.player_leds = self.player_leds;
        profile.mic_enabled = self.microphone.enabled;

        profile.stick_left_curve = self.sticks.left_curve.clone();
        profile.stick_right_curve = self.sticks.right_curve.clone();
        profile.stick_left_deadzone = self.sticks.left_deadzone;
        profile.stick_right_deadzone = self.sticks.right_deadzone;
        profile.stick_left_outer_deadzone = self.sticks.left_outer_deadzone;
        profile.stick_right_outer_deadzone = self.sticks.right_outer_deadzone;
        profile.stick_left_invert_x = self.sticks.left_invert_x;
        profile.stick_left_invert_y = self.sticks.left_invert_y;
        profile.stick_right_invert_x = self.sticks.right_invert_x;
        profile.stick_right_invert_y = self.sticks.right_invert_y;
        profile.stick_swap = self.sticks.swap;

        profile.trigger_left_config = self.triggers.left.clone();
        profile.trigger_right_config = self.triggers.right.clone();

        profile.gyro = self.gyro.processor.clone();
        profile.touchpad_enabled = !matches!(self.touchpad.mode, TouchpadMode::Disabled);
        profile.touchpad_mode = self.touchpad.mode;
        profile.touchpad_show_overlay = self.touchpad.show_overlay;
        profile.touchpad_tap_to_click = self.touchpad.tap_to_click;
        profile.touchpad_natural_scrolling = self.touchpad.natural_scrolling;
        profile.touchpad_sensitivity = self.touchpad.sensitivity;

        profile.haptic_pattern = self.haptic_state.pattern;
        profile.haptic_strength = self.haptic_state.strength;
        profile.haptic_speed = self.haptic_state.speed;

        if let Some(ref ipc) = self.ipc.clone() {
            let _ = mlock(ipc).save_profile(profile.clone());
        } else {
            let _ = self.profile_manager.save_profile(profile);
        }
    }

    pub(crate) fn check_controller_connection(&mut self) {
        if self.firmware.updating {
            return;
        }

        if let Some(ipc) = self.ipc.clone() {
            let list = mlock(&ipc).list_devices();
            match list {
                Err(_) => {
                    self.stop_input_polling();
                    self.disconnect_controller();
                }
                Ok(list) => {
                    self.devices = list;
                    if self.devices.is_empty() {
                        self.stop_input_polling();
                        self.disconnect_controller();
                        return;
                    }
                    let selected_present = self
                        .controller_serial
                        .as_ref()
                        .is_some_and(|id| self.devices.iter().any(|d| &d.id == id));
                    if !selected_present {
                        let first = self.devices[0].id.clone();
                        self.selected_device = None;
                        self.switch_device(&first);
                    }
                }
            }
            return;
        }

        if self.api.refresh_devices().is_err() {
            return;
        }
        let found = dualsense::discover(&self.api);
        self.devices = found.iter().enumerate().map(|(i, d)| d.info(i)).collect();

        let present = self
            .controller_serial
            .as_ref()
            .is_some_and(|id| found.iter().any(|d| &d.id == id));
        if !present && self.controller_serial.is_some() {
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
            let available = matches!(mlock(&client).get_controller_info(), Ok(Some(_)));
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
        if self.firmware.checking_latest {
            return;
        }

        let Some(pid) = self.controller_product_id else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let downloader = self.firmware.downloader.clone();

        self.firmware.checking_latest = true;
        self.firmware.progress_rx = Some(rx);
        thread::spawn(move || match downloader.get_latest_version() {
            Ok((ds_ver, dse_ver)) => {
                let ver = if pid == DS_PID { ds_ver } else { dse_ver };
                let _ = tx.send(ProgressUpdate::LatestVersion(ver));
            }
            Err(e) => {
                let _ = tx.send(ProgressUpdate::Error(format!(
                    "Version check failed: {}",
                    e
                )));
            }
        });
    }

    pub(crate) fn refresh_firmware_info(&mut self) {
        let result = if let Some(ref ipc) = self.ipc.clone() {
            mlock(ipc).get_firmware_info()
        } else if let Some(ref ctrl) = self.controller {
            mlock(ctrl).get_firmware_info()
        } else {
            self.error_message = "No controller available".into();
            return;
        };

        match result {
            Ok((ver, date, time)) => {
                self.firmware.current_version = Some(ver);
                self.status_message = format!("Firmware: 0x{:04X}  ({} {})", ver, date, time);
                self.firmware.build_date = Some(date);
                self.firmware.build_time = Some(time);
                self.error_message.clear();
            }
            Err(e) => {
                self.error_message = format!("Failed to read firmware: {}", e);
            }
        }
    }

    pub(crate) fn flash_latest(&mut self) {
        self.stop_input_polling();

        if !self.acquire_direct_fw() {
            return;
        }

        let pid = self.controller_product_id.unwrap_or(DS_PID);
        let ctrl = Arc::clone(self.controller.as_ref().unwrap());
        let (tx, rx) = mpsc::channel();
        let downloader = self.firmware.downloader.clone();

        self.firmware.progress_rx = Some(rx);
        self.firmware.updating = true;
        self.firmware.progress = 0;
        self.firmware.last_flash_result = None;
        self.firmware.status = "Downloading latest firmware...".to_string();

        let handle = thread::spawn(move || {
            {
                let c = mlock(&ctrl);
                c.set_update_mode(true);
            }

            let mut ctrl = mlock(&ctrl);

            let fw_data = match downloader.download_latest_firmware(pid, {
                let tx = tx.clone();
                move |p| {
                    let _ = tx.send(ProgressUpdate::Progress(p / 2));
                }
            }) {
                Ok(d) => d,
                Err(e) => {
                    let _ = tx.send(ProgressUpdate::Error(e.to_string()));
                    return;
                }
            };

            ctrl.set_update_mode(false);

            let _ = tx.send(ProgressUpdate::Status("Flashing...".to_string()));
            let tx_flash = tx.clone();
            let result = ctrl.update_firmware(&fw_data, move |p| {
                let _ = tx_flash.send(ProgressUpdate::Progress(50 + p / 2));
            });

            ctrl.set_update_mode(false);

            match result {
                Ok(_) => {
                    let _ = tx.send(ProgressUpdate::Complete);
                }
                Err(e) => {
                    let _ = tx.send(ProgressUpdate::Error(e.to_string()));
                }
            }
        });
        self.firmware.thread = Some(handle);
    }

    pub(crate) fn flash_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .set_title("Select firmware file")
            .add_filter("Firmware binary", &["bin"])
            .pick_file()
        else {
            return;
        };

        self.firmware.is_last_flash_file = true;
        let fw_data = match std::fs::read(&path) {
            Ok(d) => d,
            Err(e) => {
                self.error_message = format!("Failed to read file: {}", e);
                return;
            }
        };

        self.stop_input_polling();

        if !self.acquire_direct_fw() {
            return;
        }

        let ctrl = Arc::clone(self.controller.as_ref().unwrap());
        let (tx, rx) = mpsc::channel();

        self.firmware.progress_rx = Some(rx);
        self.firmware.updating = true;
        self.firmware.progress = 0;
        self.firmware.last_flash_result = None;
        self.firmware.status = "Flasing from file...".to_string();

        let handle = thread::spawn(move || {
            {
                let c = mlock(&ctrl);
                c.set_update_mode(true);
            }

            let mut ctrl = mlock(&ctrl);

            ctrl.set_update_mode(false);

            let tx_flash = tx.clone();
            let result = ctrl.update_firmware(&fw_data, move |p| {
                let _ = tx_flash.send(ProgressUpdate::Progress(50 + p / 2));
            });

            ctrl.set_update_mode(false);

            match result {
                Ok(_) => {
                    let _ = tx.send(ProgressUpdate::Complete);
                }
                Err(e) => {
                    let _ = tx.send(ProgressUpdate::Error(e.to_string()));
                }
            }
        });
        self.firmware.thread = Some(handle);
    }

    pub(crate) fn apply_input_transform(&mut self) {
        let mut t = self
            .current_profile
            .as_ref()
            .map(|p| p.to_input_transform())
            .unwrap_or_default();
        t.left_curve = self.sticks.left_curve.clone();
        t.right_curve = self.sticks.right_curve.clone();
        t.left_deadzone = self.sticks.left_deadzone;
        t.right_deadzone = self.sticks.right_deadzone;
        t.left_outer_deadzone = self.sticks.left_outer_deadzone;
        t.right_outer_deadzone = self.sticks.right_outer_deadzone;
        t.left_invert_x = self.sticks.left_invert_x;
        t.left_invert_y = self.sticks.left_invert_y;
        t.right_invert_x = self.sticks.right_invert_x;
        t.right_invert_y = self.sticks.right_invert_y;
        t.stick_swap = self.sticks.swap;
        t.touchpad_mode = self.touchpad.mode;
        t.trigger_left = self.triggers.left.deadband.clone();
        t.trigger_right = self.triggers.right.deadband.clone();

        self.input_transform = t.clone();

        if let Some(be) = self.backend() {
            be.set_input_transform(t);
        }
    }

    pub(crate) fn create_profile(&mut self, name: &str) -> bool {
        if let Err(e) = ProfileManager::validate_profile_name(name) {
            self.error_message = e.to_string();
            return false;
        }
        if name.trim().is_empty() {
            return false;
        }
        if self.profile_manager.profile_exists(name) {
            self.error_message = format!("Profile '{}' already exists", name);
            return false;
        }

        let mut p = Profile {
            name: name.to_string(),
            ..Default::default()
        };
        if let Some(cur) = &self.current_profile {
            let mut clone = cur.clone();
            clone.name = name.to_string();
            p = clone;
        }
        if self.profile_manager.save_profile(&p).is_err() {
            return false;
        }
        if let Some(ref ipc) = self.ipc.clone() {
            let _ = mlock(ipc).save_profile(p.clone());
        }
        self.load_profile(&p);
        true
    }

    pub(crate) fn delete_profile(&mut self, name: &str) -> bool {
        if name == "Default" {
            return false;
        }
        let result = if let Some(ref ipc) = self.ipc.clone() {
            mlock(ipc).delete_profile(name).is_ok()
        } else {
            self.profile_manager.delete_profile(name).is_ok()
        };
        if result
            && let Some(cur) = self.current_profile.as_ref()
            && cur.name == name
        {
            let default = self.profile_manager.ensure_default_exists();
            self.load_profile(&default);
        }
        result
    }

    pub(crate) fn using_daemon(&self) -> bool {
        self.ipc.is_some()
    }

    pub(crate) fn daemon_alive(&mut self) -> bool {
        if self.last_daemon_probe.elapsed() >= Duration::from_millis(1500) {
            self.last_daemon_probe = Instant::now();
            let addr = crate::ipc::daemon_endpoint();
            self.daemon_alive_cached = IpcClient::try_connect(&addr).is_some();
        }
        self.daemon_alive_cached
    }

    pub(crate) fn refresh_daemon_state(&mut self) {
        self.last_daemon_probe = Instant::now() - Duration::from_secs(10);
        let _ = self.daemon_alive();
    }

    pub(crate) fn start_daemon_process(&mut self) {
        if self.daemon_alive() {
            self.status_message = "Daemon already running".into();
            return;
        }
        let exe = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                self.error_message = format!("Cannot locate ds4u binary: {}", e);
                return;
            }
        };
        match std::process::Command::new(&exe)
            .arg("--daemon")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
        {
            Ok(_) => {
                self.status_message = "Daemon starting...".into();
                self.error_message.clear();
                std::thread::sleep(Duration::from_millis(250));
                self.refresh_daemon_state();
            }
            Err(e) => {
                self.error_message = format!("Failed to start daemon: {}", e);
            }
        }
    }

    pub(crate) fn stop_daemon_process(&mut self) {
        let addr = crate::ipc::daemon_endpoint();
        let mut client = match IpcClient::try_connect(&addr) {
            Some(c) => c,
            None => {
                self.error_message = "Daemon is not running".into();
                self.daemon_alive_cached = false;
                return;
            }
        };
        if self.ipc.is_some() {
            self.stop_input_polling();
            self.disconnect_controller();
        }
        match client.shutdown() {
            Ok(_) => {
                self.status_message = "Daemon stopped".into();
                self.error_message.clear();
            }
            Err(e) => {
                self.error_message = format!("Shutdown failed: {}", e);
            }
        }
        self.daemon_alive_cached = false;
        self.last_daemon_probe = Instant::now();
    }

    pub(crate) fn attach_daemon(&mut self) {
        let addr = crate::ipc::daemon_endpoint();
        let client = match IpcClient::try_connect(&addr) {
            Some(c) => Arc::new(Mutex::new(c)),
            None => {
                self.error_message = "Daemon is not running".into();
                return;
            }
        };
        self.stop_input_polling();
        self.controller = None;
        self.battery_info = None;
        self.controller_serial = None;
        self.controller_is_bt = None;
        self.controller_product_id = None;
        let has_dev = matches!(mlock(&client).get_controller_info(), Ok(Some(_)));
        if has_dev {
            self.connect_via_daemon(client);
        } else {
            self.ipc = Some(client);
            self.status_message = "Attached to daemon (waiting for device)".into();
            self.error_message.clear();
        }
    }

    pub(crate) fn detach_daemon(&mut self) {
        if self.ipc.is_none() {
            return;
        }
        self.stop_input_polling();
        self.disconnect_controller();
        self.status_message = "Detached from daemon".into();
    }
}
