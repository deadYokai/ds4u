use std::{
    fs, io::{BufRead, BufReader, Write}, os::unix::net::{UnixListener, UnixStream}, sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, thread::{self, sleep}, time::{Duration, Instant}
};

use hidapi::HidApi;

use crate::{
    common::LightbarEffect, dualsense::DualSense, ipc::{socket_path, DaemonCommand, DaemonResponse, IpcClient}, profiles::ProfileManager, settings::SettingsManager, transform::InputTransform
};

const TAG: &str = "[ds4u daemon]";

pub struct DaemonManager {
    client: Option<Arc<Mutex<IpcClient>>>,
}

impl DaemonManager {
    pub fn new() -> Self {
        let path = socket_path();
        let client = IpcClient::try_connect(&path)
            .map(|c| Arc::new(Mutex::new(c)));
        Self { client }
    }

    pub fn is_active(&self) -> bool {
        self.client.is_some()
    }

    pub fn connect_new_client(&self) -> Option<Arc<Mutex<IpcClient>>> {
        let path = socket_path();
        IpcClient::try_connect(&path).map(|c| Arc::new(Mutex::new(c)))
    }

    pub fn client(&self) -> Option<Arc<Mutex<IpcClient>>> {
        self.client.clone()
    }
    pub fn set_update_in_progress(&mut self, active: bool) {
        if let Some(ref arc) = self.client {
            let _ = arc.lock().unwrap().set_update_mode(active);
        }
    }
}

struct DaemonState {
    device: Mutex<Option<DualSense>>,
    update_in_progress: AtomicBool,
    active_transform: Mutex<InputTransform>,
    active_effect: Mutex<LightbarEffect>,
    lightbar_color: Mutex<(u8, u8, u8, u8)>,
}

impl DaemonState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            device: Mutex::new(None),
            update_in_progress: AtomicBool::new(false),
            active_transform: Mutex::new(InputTransform::default()),
            active_effect: Mutex::new(LightbarEffect::None),
            lightbar_color: Mutex::new((0, 128, 255, 255))
        })
    }
}

pub fn run_daemon() {
    let path = socket_path();

    if path.exists() {
        let _ = fs::remove_file(&path);
    }
    
    let listener = UnixListener::bind(&path)
        .unwrap_or_else(|e| panic!("Cannot bind {}: {}", path.display(), e));

    println!("{} listening on {}", TAG, path.display());

    let state = DaemonState::new();

    {
        let sm = SettingsManager::new();
        let settings = sm.load();
        let pm = ProfileManager::new();

        let name = if settings.profile.is_empty() {
            "Default".to_string()
        } else {
            settings.profile
        };

        let profile = if pm.profile_exists(&name) {
            pm.load_profile(&name).ok()
        } else {
            Some(pm.ensure_default_exists())
        };

        if let Some(p) = profile {
            *state.active_transform.lock().unwrap() = p.to_input_transform();
            println!("{} autoloaded profile '{}'", TAG, p.name);
        }
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || device_connection_loop(s));
    }

    {
        let s = Arc::clone(&state);
        thread::spawn(move || effect_loop(s));
    }

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                let state = Arc::clone(&state);
                thread::spawn(move || handle_client(s, state));
            }
            Err(e) => eprintln!("{} accept error: {}", TAG, e)
        }
    }

}

fn device_connection_loop(state: Arc<DaemonState>) {
    loop {
        if !state.update_in_progress.load(Ordering::Relaxed) {
            let mut dev = state.device.lock().unwrap();
            if dev.is_none()
                && let Ok(api) = HidApi::new()
            {
                match DualSense::new(&api, None) {
                    Ok(ds) => {
                        println!("{} controller connected: {}", TAG, ds.serial());
                        *dev = Some(ds)
                    }
                    Err(_) => {}
                }
            }
        }
        sleep(Duration::from_secs(2));
    }
}

fn handle_client(stream: UnixStream, state: Arc<DaemonState>) {
    let write_half = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return
    };

    let mut reader = BufReader::new(stream);
    let mut writer = write_half;

    let send = |w: &mut UnixStream, resp: DaemonResponse| {
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
                send(&mut writer, DaemonResponse::Error { message: e.to_string() });
                continue;
            }
        };

        match cmd {
            DaemonCommand::Ping => { send(&mut writer, DaemonResponse::Pong); }

            DaemonCommand::SetUpdateMode { active } => {
                if active {
                    state.update_in_progress.store(true, Ordering::SeqCst);
                    *state.device.lock().unwrap() = None;
                    println!("{} device released for firmware update", TAG);
                } else {
                    state.update_in_progress.store(false, Ordering::SeqCst);
                    println!("{} firmware update done, device will reconnect", TAG);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetInputTransform { transform } => {
                *state.active_transform.lock().unwrap() = transform;
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ClearInputTransform => {
                *state.active_transform.lock().unwrap() = InputTransform::default();
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetLightbarEffect { effect } => {
                let restoring = matches!(effect, LightbarEffect::None);
                *state.active_effect.lock().unwrap() = effect;

                if restoring {
                    let (r, g, b, br) = *state.lightbar_color.lock().unwrap();
                    if let Some(ds) = state.device.lock().unwrap().as_mut() {
                        let _ = ds.set_lightbar(r, g, b, br);
                    }
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            cmd => {
                if state.update_in_progress.load(Ordering::Relaxed) {
                    send(&mut writer, DaemonResponse::Error { 
                        message: "Firmware update in progress".to_string()
                    });
                    continue;
                }

                if let DaemonCommand::SetLightbar { r, g, b, brightness } = &cmd {
                    *state.lightbar_color.lock().unwrap() = (*r, *g, *b, *brightness);
                    if !matches!(*state.active_effect.lock().unwrap(), LightbarEffect::None) {
                        send(&mut writer, DaemonResponse::Ok);
                        continue;
                    }
                }

                let transform = state.active_transform.lock().unwrap().clone();
                let mut dev = state.device.lock().unwrap();
                match dev.as_mut() {
                    None => send(&mut writer, DaemonResponse::NoDevice),
                    Some(ds) => {
                        let resp = dispatch(ds, cmd, &transform);
                        let failed = matches!(&resp, DaemonResponse::Error { .. });
                        send(&mut writer, resp);
                        if failed {
                            println!("{} device error - dropping handle", TAG);
                            *dev = None;
                        }
                    }
                }
            }
        }
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c  = v * s;
    let x  = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m  = v - c;
    let (r, g, b) = if      h < 60.0  { (c, x, 0.0) }
                    else if h < 120.0 { (x, c, 0.0) }
                    else if h < 180.0 { (0.0, c, x) }
                    else if h < 240.0 { (0.0, x, c) }
                    else if h < 300.0 { (x, 0.0, c) }
                    else              { (c, 0.0, x) };
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

        let effect = state.active_effect.lock().unwrap().clone();
        if matches!(effect, LightbarEffect::None) {
            continue;
        }

        if state.update_in_progress.load(Ordering::Relaxed) {
            continue;
        }

        let t = start.elapsed().as_secs_f32();
        let (base_r, base_g, base_b, base_br) = *state.lightbar_color.lock().unwrap();

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
                if (t * speed * 2.0) as u32 % 2 == 0 {
                    (base_r, base_g, base_b)
                } else {
                    (0, 0, 0)
                }
            }
            LightbarEffect::None => unreachable!(),
        };

        if let Ok(mut dev) = state.device.try_lock() {
            if let Some(ds) = dev.as_mut() {
                let _ = ds.set_lightbar(r, g, b, base_br);
            }
        }
    }
}


fn dispatch(ds: &mut DualSense, cmd: DaemonCommand, transform: &InputTransform)
    -> DaemonResponse
{
    macro_rules! ok_or_err {
        ($e:expr) => {
            match $e {
                Ok(_)  => DaemonResponse::Ok,
                Err(e) => DaemonResponse::Error { message: e.to_string() },
            }
        };
    }

    match cmd {
        DaemonCommand::Ping => DaemonResponse::Pong,

        DaemonCommand::GetBattery => match ds.get_battery() {
            Ok(b)  => DaemonResponse::Battery(b),
            Err(e) => DaemonResponse::Error { message: e.to_string() }
        },

        DaemonCommand::GetInputState => match ds.get_input_state() {
            Ok(mut s) => {
                transform.apply(&mut s);
                DaemonResponse::InputState(s)
            }
            Err(e) => DaemonResponse::Error { message: e.to_string() },
        },

        DaemonCommand::GetFirmwareInfo => match ds.get_firmware_info() {
            Ok((v, d, t)) => DaemonResponse::FirmwareInfo {
                version: v, build_date: d, build_time: t,
            },
            Err(e) => DaemonResponse::Error { message: e.to_string() },
        },

        DaemonCommand::GetControllerInfo => DaemonResponse::ControllerInfo {
            serial: ds.serial().to_string(),
            product_id: ds.product_id(),
            is_bt: ds.is_bluetooth()
        },

        DaemonCommand::SetLightbar { r, g, b, brightness } =>
            ok_or_err!(ds.set_lightbar(r, g, b, brightness)),

        DaemonCommand::SetLightbarEnabled { enabled } =>
            ok_or_err!(ds.set_lightbar_enabled(enabled)),

        DaemonCommand::SetPlayerLeds { leds } =>
            ok_or_err!(ds.set_player_leds(leds)),

        DaemonCommand::SetMic { enabled } =>
            ok_or_err!(ds.set_mic(enabled)),

        DaemonCommand::SetMicLed { state } =>
            ok_or_err!(ds.set_mic_led(state)),

        DaemonCommand::SetTriggerOff =>
            ok_or_err!(ds.set_trigger_off()),

        DaemonCommand::SetTriggerEffect { right, left, effect_type, params } =>
            ok_or_err!(ds.set_trigger_effect(left, right, effect_type, &params)),

        DaemonCommand::SetVibration { rumble, trigger } =>
            ok_or_err!(ds.set_vibration(rumble, trigger)),

        DaemonCommand::SetSpeaker { mode } =>
            ok_or_err!(ds.set_speaker(&mode)),

        DaemonCommand::SetVolume { volume } =>
            ok_or_err!(ds.set_volume(volume)),

        DaemonCommand::SetLightbarEffect { .. } => unreachable!(),

        DaemonCommand::SetUpdateMode { .. } => unreachable!(),
        DaemonCommand::SetInputTransform { .. } => unreachable!(),
        DaemonCommand::ClearInputTransform => unreachable!()
    }
}

