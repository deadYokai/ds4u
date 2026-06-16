use std::{
    io::{BufRead, BufReader, Write},
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, sleep},
    time::{Duration, Instant},
};

use hidapi::HidApi;

use crate::{
    common::{HapticPattern, LightbarEffect},
    dualsense::{DualSense, HAPTICS_PACKET_FRAMES, HAPTICS_SAMPLE_RATE},
    haptics_stream::generate_packet,
    ipc::{
        DaemonCommand, DaemonResponse, DaemonStream, IpcClient, addr_display, bind_daemon,
        cleanup_endpoint, daemon_endpoint,
    },
    profiles::ProfileManager,
    settings::SettingsManager,
    transform::{GyroProcessor, InputTransform},
    util::{mlock, rlock, wait_cv, wlock},
};

const TAG: &str = "[ds4u daemon]";

pub struct DaemonManager {
    client: Option<Arc<Mutex<IpcClient>>>,
}

impl DaemonManager {
    pub fn new() -> Self {
        let addr = daemon_endpoint();
        let client = IpcClient::try_connect(&addr).map(|c| Arc::new(Mutex::new(c)));
        Self { client }
    }

    pub fn is_active(&self) -> bool {
        self.client.is_some()
    }

    pub fn connect_new_client(&self) -> Option<Arc<Mutex<IpcClient>>> {
        let addr = daemon_endpoint();
        IpcClient::try_connect(&addr).map(|c| Arc::new(Mutex::new(c)))
    }

    pub fn client(&self) -> Option<Arc<Mutex<IpcClient>>> {
        self.client.clone()
    }
    pub fn set_update_in_progress(&mut self, active: bool) {
        if let Some(ref arc) = self.client {
            let _ = mlock(arc).set_update_mode(active);
        }
    }
}

struct DaemonInner {
    active_transform: InputTransform,
    active_effect: LightbarEffect,
    lightbar_color: (u8, u8, u8, u8),
    player_leds: u8,
    mic_enabled: bool,
    active_profile_name: String,
    trigger_left: Option<(u8, [u8; 10])>,
    trigger_right: Option<(u8, [u8; 10])>,
    haptic: (HapticPattern, u8, f32),
    raw_haptics: bool,
    gyro: GyroProcessor,
}

struct DaemonState {
    device: Mutex<Option<DualSense>>,
    update_in_progress: AtomicBool,
    inner: RwLock<DaemonInner>,
    hotplug: (Mutex<bool>, Condvar),
}

impl DaemonState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            device: Mutex::new(None),
            update_in_progress: AtomicBool::new(false),
            inner: RwLock::new(DaemonInner {
                active_transform: InputTransform::default(),
                active_effect: LightbarEffect::None,
                lightbar_color: (0, 128, 255, 255),
                player_leds: 1,
                mic_enabled: false,
                active_profile_name: String::new(),
                trigger_left: None,
                trigger_right: None,
                haptic: (HapticPattern::None, 0, 1.0),
                raw_haptics: false,
                gyro: GyroProcessor::default(),
            }),
            hotplug: (Mutex::new(false), Condvar::new()),
        })
    }
}

fn apply_profile_to_state(state: &Arc<DaemonState>, name: &str) -> String {
    let pm = ProfileManager::new();
    let profile = if pm.profile_exists(name) {
        pm.load_profile(name).ok()
    } else if pm.profile_exists("Default") {
        pm.load_profile("Default").ok()
    } else {
        Some(pm.ensure_default_exists())
    };

    let Some(p) = profile else {
        return String::new();
    };

    let mut inner = wlock(&state.inner);
    inner.active_transform = p.to_input_transform();
    let r = (p.lightbar_r * 255.0) as u8;
    let g = (p.lightbar_g * 255.0) as u8;
    let b = (p.lightbar_b * 255.0) as u8;
    let br = p.lightbar_brightness as u8;
    inner.lightbar_color = (r, g, b, br);
    inner.player_leds = p.player_leds;
    inner.mic_enabled = p.mic_enabled;

    use crate::common::TriggerMode;
    inner.trigger_left = match p.trigger_left_config.mode {
        TriggerMode::Off => None,
        _ => Some(p.trigger_left_config.to_effect()),
    };
    inner.trigger_right = match p.trigger_right_config.mode {
        TriggerMode::Off => None,
        _ => Some(p.trigger_right_config.to_effect()),
    };

    inner.haptic = (p.haptic_pattern, p.haptic_strength, p.haptic_speed);
    inner.gyro = p.to_gyro_processor();

    p.name.clone()
}

fn push_triggers_to_device(state: &Arc<DaemonState>) {
    let (left, right) = {
        let inner = rlock(&state.inner);
        (inner.trigger_left, inner.trigger_right)
    };
    if let Some(ds) = mlock(&state.device).as_mut() {
        let l = left.or(Some((0x05, [0u8; 10])));
        let r = right.or(Some((0x05, [0u8; 10])));
        let _ = ds.set_trigger_effects(l, r);
    }
}

pub fn run_daemon() {
    let addr = daemon_endpoint();

    cleanup_endpoint(&addr);

    let listener = bind_daemon(&addr)
        .unwrap_or_else(|e| panic!("{} cannot bind {}: {}", TAG, addr_display(&addr), e));

    println!("{} listening on {}", TAG, addr_display(&addr));

    let state = DaemonState::new();

    {
        let sm = SettingsManager::new();
        let settings = sm.load();
        let name = if settings.profile.is_empty() {
            "Default"
        } else {
            &settings.profile
        };
        let loaded = apply_profile_to_state(&state, name);
        wlock(&state.inner).active_profile_name = loaded.clone();
        println!("{} profile '{}' loaded", TAG, loaded);
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || device_connection_loop(s));
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || hotplug_thread(s));
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || effect_loop(s));
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || haptic_loop(s));
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || raw_haptic_loop(s));
    }

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_client(s, state));
            }
            Err(e) => eprintln!("{} accept error: {}", TAG, e),
        }
    }
}

fn device_connection_loop(state: Arc<DaemonState>) {
    loop {
        if !state.update_in_progress.load(Ordering::Relaxed) {
            let mut dev = mlock(&state.device);
            if dev.is_none()
                && let Ok(api) = HidApi::new()
                && let Ok(mut ds) = DualSense::new(&api, None)
            {
                println!("{} controller connected: {}", TAG, ds.serial());

                let snap = {
                    let i = rlock(&state.inner);
                    (
                        i.lightbar_color,
                        i.player_leds,
                        i.mic_enabled,
                        i.trigger_left,
                        i.trigger_right,
                    )
                };
                let ((r, g, b, br), leds, mic, tl, tr) = snap;
                let _ = ds.set_lightbar(r, g, b, br);
                let _ = ds.set_player_leds(leds);
                let _ = ds.set_mic(mic);
                let l = tl.or(Some((0x05, [0u8; 10])));
                let r2 = tr.or(Some((0x05, [0u8; 10])));
                let _ = ds.set_trigger_effects(l, r2);

                *dev = Some(ds);
            }

            drop(dev);
        }

        let (lock, cvar) = &state.hotplug;
        let mut signaled = mlock(lock);
        if !*signaled {
            let (g, _timed_out) = wait_cv(cvar, signaled, Duration::from_secs(5));
            signaled = g;
        }
        *signaled = false;
    }
}

fn hotplug_thread(state: Arc<DaemonState>) {
    use std::os::fd::AsRawFd;

    let socket = match udev::MonitorBuilder::new()
        .and_then(|b| b.match_subsystem("hidraw"))
        .and_then(|b| b.listen())
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{} udev monitor unavailable: {} (falling back to polling)",
                TAG, e
            );
            return;
        }
    };
    let fd = socket.as_raw_fd();

    println!("{} udev hotplug watcher active", TAG);
    let (lock, cvar) = &state.hotplug;

    loop {
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let r = unsafe { libc::poll(&mut pfd, 1, -1) };
        if r < 0 {
            sleep(Duration::from_secs(1));
            continue;
        }

        let mut interesting = false;
        for ev in socket.iter() {
            use udev::EventType;
            if matches!(
                ev.event_type(),
                EventType::Add | EventType::Remove | EventType::Bind | EventType::Unbind
            ) {
                interesting = true;
            }
        }

        if interesting {
            *mlock(lock) = true;
            cvar.notify_all();
        }
    }
}

fn handle_client(stream: DaemonStream, state: Arc<DaemonState>) {
    let write_half = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut reader = BufReader::new(stream);
    let mut writer = write_half;

    let send = |w: &mut DaemonStream, resp: DaemonResponse| {
        if let Ok(mut line) = serde_json::to_string(&resp) {
            line.push('\n');
            let _ = w.write_all(line.as_bytes());
        }
    };

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }

        let cmd: DaemonCommand = match serde_json::from_str(line.trim()) {
            Ok(c) => c,
            Err(e) => {
                send(
                    &mut writer,
                    DaemonResponse::Error {
                        message: e.to_string(),
                    },
                );
                continue;
            }
        };

        match cmd {
            DaemonCommand::Ping => {
                send(&mut writer, DaemonResponse::Pong);
            }

            DaemonCommand::Shutdown => {
                send(&mut writer, DaemonResponse::Ok);
                println!("{} shutdown requested via IPC", TAG);
                std::process::exit(0);
            }

            DaemonCommand::SetUpdateMode { active } => {
                if active {
                    state.update_in_progress.store(true, Ordering::SeqCst);
                    *mlock(&state.device) = None;
                    println!("{} device released for firmware update", TAG);
                } else {
                    state.update_in_progress.store(false, Ordering::SeqCst);
                    println!("{} firmware update done, device will reconnect", TAG);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetInputTransform { transform } => {
                wlock(&state.inner).active_transform = transform;
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ClearInputTransform => {
                wlock(&state.inner).active_transform = InputTransform::default();
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetLightbarEffect { effect } => {
                let restoring = matches!(effect, LightbarEffect::None);
                wlock(&state.inner).active_effect = effect;

                if restoring {
                    let (r, g, b, br) = rlock(&state.inner).lightbar_color;
                    if let Some(ds) = mlock(&state.device).as_mut() {
                        let _ = ds.set_lightbar(r, g, b, br);
                    }
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetHapticPattern {
                pattern,
                strength,
                speed,
            } => {
                let restoring = matches!(pattern, HapticPattern::None);
                wlock(&state.inner).haptic = (pattern, strength.min(7), speed.max(0.05));
                if restoring && let Some(ds) = mlock(&state.device).as_mut() {
                    let _ = ds.set_vibration(0, 0);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetRawHaptics { active } => {
                wlock(&state.inner).raw_haptics = active;
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetGyro {
                enabled,
                smoothing,
                sensitivity,
            } => {
                let mut inner = wlock(&state.inner);
                let g = &mut inner.gyro;
                g.enabled = enabled;
                g.smoothing = smoothing.clamp(0.0, 0.95);
                g.sensitivity = sensitivity.max(0.0);
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetTriggerEffects { left, right } => {
                {
                    let mut inner = wlock(&state.inner);
                    if let Some(l) = left {
                        inner.trigger_left = Some(l);
                    }
                    if let Some(r) = right {
                        inner.trigger_right = Some(r);
                    }
                }
                if state.update_in_progress.load(Ordering::Relaxed) {
                    send(
                        &mut writer,
                        DaemonResponse::Error {
                            message: "Firmware update in progress".to_string(),
                        },
                    );
                    continue;
                }
                let (l, r) = {
                    let i = rlock(&state.inner);
                    (i.trigger_left, i.trigger_right)
                };
                if let Some(ds) = mlock(&state.device).as_mut() {
                    let lf = l.or(Some((0x05, [0u8; 10])));
                    let rt = r.or(Some((0x05, [0u8; 10])));
                    let _ = ds.set_trigger_effects(lf, rt);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SwitchProfile { name } => {
                let loaded = apply_profile_to_state(&state, &name);
                wlock(&state.inner).active_profile_name = loaded.clone();
                let sm = SettingsManager::new();
                let mut settings = sm.load();
                settings.profile = loaded.clone();
                sm.save(&settings);
                {
                    let (color, leds, mic) = {
                        let i = rlock(&state.inner);
                        (i.lightbar_color, i.player_leds, i.mic_enabled)
                    };
                    let (r, g, b, br) = color;
                    if let Some(ds) = mlock(&state.device).as_mut() {
                        let _ = ds.set_lightbar(r, g, b, br);
                        let _ = ds.set_player_leds(leds);
                        let _ = ds.set_mic(mic);
                    }
                }
                push_triggers_to_device(&state);
                println!("{} switched to profile '{}'", TAG, loaded);
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ReloadProfile => {
                let name = rlock(&state.inner).active_profile_name.clone();
                let name = if name.is_empty() {
                    "Default".to_string()
                } else {
                    name
                };
                apply_profile_to_state(&state, &name);
                push_triggers_to_device(&state);
                println!("{} reloaded profile '{}'", TAG, name);
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ListProfiles => {
                let pm = ProfileManager::new();
                let profiles = pm.list_profiles().into_iter().map(|p| p.name).collect();
                send(&mut writer, DaemonResponse::ProfileList { profiles });
            }

            DaemonCommand::SaveProfile { profile } => {
                let pm = ProfileManager::new();
                match pm.save_profile(&profile) {
                    Ok(_) => send(&mut writer, DaemonResponse::Ok),
                    Err(e) => send(
                        &mut writer,
                        DaemonResponse::Error {
                            message: e.to_string(),
                        },
                    ),
                }
            }

            DaemonCommand::DeleteProfile { name } => {
                let pm = ProfileManager::new();
                match pm.delete_profile(&name) {
                    Ok(_) => send(&mut writer, DaemonResponse::Ok),
                    Err(e) => send(
                        &mut writer,
                        DaemonResponse::Error {
                            message: e.to_string(),
                        },
                    ),
                }
            }

            DaemonCommand::GetActiveProfile => {
                let name = rlock(&state.inner).active_profile_name.clone();
                send(&mut writer, DaemonResponse::ActiveProfile { name });
            }

            cmd @ (DaemonCommand::GetBattery
            | DaemonCommand::GetInputState
            | DaemonCommand::GetFirmwareInfo
            | DaemonCommand::GetControllerInfo) => {
                let transform = rlock(&state.inner).active_transform.clone();
                let mut dev = mlock(&state.device);
                match dev.as_mut() {
                    None => send(&mut writer, DaemonResponse::NoDevice),
                    Some(ds) => {
                        let resp = dispatch(ds, cmd, &transform);
                        send(&mut writer, resp);
                    }
                }
            }

            cmd => {
                if state.update_in_progress.load(Ordering::Relaxed) {
                    send(
                        &mut writer,
                        DaemonResponse::Error {
                            message: "Firmware update in progress".to_string(),
                        },
                    );
                    continue;
                }

                if let DaemonCommand::SetLightbar {
                    r,
                    g,
                    b,
                    brightness,
                } = &cmd
                {
                    let mut inner = wlock(&state.inner);
                    inner.lightbar_color = (*r, *g, *b, *brightness);
                    if !matches!(inner.active_effect, LightbarEffect::None) {
                        send(&mut writer, DaemonResponse::Ok);
                        continue;
                    }
                }

                if let DaemonCommand::SetTriggerEffect {
                    left,
                    right,
                    effect_type,
                    params,
                } = &cmd
                {
                    let mut inner = wlock(&state.inner);
                    let new_val = if *effect_type == 0x05 {
                        None
                    } else {
                        Some((*effect_type, *params))
                    };
                    if *left {
                        inner.trigger_left = new_val;
                    }
                    if *right {
                        inner.trigger_right = new_val;
                    }
                } else if let DaemonCommand::SetTriggerOff = &cmd {
                    let mut inner = wlock(&state.inner);
                    inner.trigger_left = None;
                    inner.trigger_right = None;
                }

                let transform = rlock(&state.inner).active_transform.clone();
                let mut dev = mlock(&state.device);
                match dev.as_mut() {
                    None => {
                        let (l, c) = &state.hotplug;
                        *mlock(l) = true;
                        c.notify_all();
                        send(&mut writer, DaemonResponse::NoDevice);
                    }
                    Some(ds) => {
                        let resp = dispatch(ds, cmd, &transform);
                        let failed = matches!(&resp, DaemonResponse::Error { .. });
                        send(&mut writer, resp);
                        if failed {
                            println!("{} device error - dropping handle", TAG);
                            *dev = None;
                            let (l, c) = &state.hotplug;
                            *mlock(l) = true;
                            c.notify_all();
                        }
                    }
                }
            }
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

fn effect_loop(state: Arc<DaemonState>) {
    let start = Instant::now();
    loop {
        sleep(Duration::from_millis(33));

        let effect = rlock(&state.inner).active_effect.clone();
        if matches!(effect, LightbarEffect::None) {
            continue;
        }

        if state.update_in_progress.load(Ordering::Relaxed) {
            continue;
        }

        let t = start.elapsed().as_secs_f32();
        let (base_r, base_g, base_b, base_br) = rlock(&state.inner).lightbar_color;

        let (r, g, b) = match effect {
            LightbarEffect::Breath { speed } => {
                let factor = ((t * speed * std::f32::consts::TAU).sin() * 0.5 + 0.5).max(0.0);
                (
                    (base_r as f32 * factor) as u8,
                    (base_g as f32 * factor) as u8,
                    (base_b as f32 * factor) as u8,
                )
            }
            LightbarEffect::Rainbow { speed } => {
                let hue = (t * speed * 360.0) % 360.0;
                hsv_to_rgb(hue, 1.0, 1.0)
            }
            LightbarEffect::Strobe { speed } => {
                if ((t * speed * 2.0) as u32).is_multiple_of(2) {
                    (base_r, base_g, base_b)
                } else {
                    (0, 0, 0)
                }
            }
            LightbarEffect::None => unreachable!(),
        };

        if let Ok(mut dev) = state.device.try_lock()
            && let Some(ds) = dev.as_mut()
        {
            let _ = ds.set_lightbar(r, g, b, base_br);
        }
    }
}

fn haptic_loop(state: Arc<DaemonState>) {
    let start = Instant::now();
    let mut last_amp: u8 = 255;
    loop {
        sleep(Duration::from_millis(40));
        if state.update_in_progress.load(Ordering::Relaxed) {
            continue;
        }

        let (raw, (pattern, strength, speed)) = {
            let g = rlock(&state.inner);
            (g.raw_haptics, g.haptic)
        };
        if raw {
            if last_amp != 0 {
                let mut dev = mlock(&state.device);
                if let Some(ds) = dev.as_mut() {
                    let _ = ds.set_rumble(0, 0);
                }
                last_amp = 0;
            }
            continue;
        }

        if matches!(pattern, HapticPattern::None) {
            if last_amp != 0 {
                let mut dev = mlock(&state.device);
                if let Some(ds) = dev.as_mut() {
                    let _ = ds.set_rumble(0, 0);
                }
                last_amp = 0;
            }
            continue;
        }

        let t = start.elapsed().as_secs_f32();
        let s = (strength.min(7) as f32 / 7.0).clamp(0.0, 1.0);
        let intensity = match pattern {
            HapticPattern::None => 0.0,
            HapticPattern::Constant => s,
            HapticPattern::Pulse => {
                let phase = (t * speed) % 1.0;
                if phase < 0.5 { s } else { 0.0 }
            }
            HapticPattern::Ramp => {
                let phase = (t * speed) % 1.0;
                s * phase
            }
            HapticPattern::Wave => {
                let v = (t * speed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
                s * v
            }
        };

        let amp = (intensity.clamp(0.0, 1.0) * 255.0).round() as u8;
        if amp != last_amp {
            let mut dev = mlock(&state.device);
            if let Some(ds) = dev.as_mut() {
                let _ = ds.set_rumble(amp, amp);
            }
            last_amp = amp;
        }
    }
}

fn raw_haptic_loop(state: Arc<DaemonState>) {
    let start = Instant::now();
    let period = Duration::from_secs_f32(HAPTICS_PACKET_FRAMES as f32 / HAPTICS_SAMPLE_RATE as f32);
    loop {
        sleep(period);
        if state.update_in_progress.load(Ordering::Relaxed) {
            continue;
        }
        let (raw, (pattern, strength, speed)) = {
            let g = rlock(&state.inner);
            (g.raw_haptics, g.haptic)
        };
        if !raw || matches!(pattern, HapticPattern::None) {
            continue;
        }
        let t0 = start.elapsed().as_secs_f32();
        let packet = generate_packet(pattern, strength, speed, t0);
        let mut dev = mlock(&state.device);
        if let Some(ds) = dev.as_mut().filter(|d| d.is_bluetooth()) {
            let _ = ds.set_haptics(&packet);
        }
    }
}

fn dispatch(ds: &mut DualSense, cmd: DaemonCommand, transform: &InputTransform) -> DaemonResponse {
    macro_rules! ok_or_err {
        ($e:expr) => {
            match $e {
                Ok(_) => DaemonResponse::Ok,
                Err(e) => DaemonResponse::Error {
                    message: e.to_string(),
                },
            }
        };
    }

    match cmd {
        DaemonCommand::Ping => DaemonResponse::Pong,

        DaemonCommand::GetBattery => match ds.get_battery() {
            Ok(b) => DaemonResponse::Battery(b),
            Err(e) => DaemonResponse::Error {
                message: e.to_string(),
            },
        },

        DaemonCommand::GetInputState => match ds.get_input_state() {
            Ok(mut s) => {
                transform.apply(&mut s);
                DaemonResponse::InputState(s)
            }
            Err(e) => DaemonResponse::Error {
                message: e.to_string(),
            },
        },

        DaemonCommand::GetFirmwareInfo => match ds.get_firmware_info() {
            Ok((v, d, t)) => DaemonResponse::FirmwareInfo {
                version: v,
                build_date: d,
                build_time: t,
            },
            Err(e) => DaemonResponse::Error {
                message: e.to_string(),
            },
        },

        DaemonCommand::GetControllerInfo => DaemonResponse::ControllerInfo {
            serial: ds.serial().to_string(),
            product_id: ds.product_id(),
            is_bt: ds.is_bluetooth(),
        },

        DaemonCommand::SetLightbar {
            r,
            g,
            b,
            brightness,
        } => ok_or_err!(ds.set_lightbar(r, g, b, brightness)),

        DaemonCommand::SetLightbarEnabled { enabled } => {
            ok_or_err!(ds.set_lightbar_enabled(enabled))
        }

        DaemonCommand::SetPlayerLeds { leds } => ok_or_err!(ds.set_player_leds(leds)),

        DaemonCommand::SetMic { enabled } => ok_or_err!(ds.set_mic(enabled)),

        DaemonCommand::SetMicLed { state } => ok_or_err!(ds.set_mic_led(state)),

        DaemonCommand::SetTriggerOff => ok_or_err!(ds.set_trigger_off()),

        DaemonCommand::SetTriggerEffect {
            right,
            left,
            effect_type,
            params,
        } => ok_or_err!(ds.set_trigger_effect(left, right, effect_type, &params)),

        DaemonCommand::SetVibration { rumble, trigger } => {
            ok_or_err!(ds.set_vibration(rumble, trigger))
        }

        DaemonCommand::SetRumble { left, right } => {
            ok_or_err!(ds.set_rumble(left, right))
        }

        DaemonCommand::SetSpeaker { mode } => ok_or_err!(ds.set_speaker(&mode)),

        DaemonCommand::SetVolume { volume } => ok_or_err!(ds.set_volume(volume)),

        _ => unreachable!(),
    }
}
