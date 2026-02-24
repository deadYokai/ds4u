use std::{
    fs,
    io::{Write, BufReader, BufRead},
    os::unix::net::{UnixListener, UnixStream},
    sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex},
    thread::{self, sleep},
    time::Duration
};

use hidapi::HidApi;

use crate::{
    dualsense::{DualSense},
    ipc::{socket_path, DaemonCommand, DaemonResponse, IpcClient},
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
    update_in_progress: AtomicBool
}

impl DaemonState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            device: Mutex::new(None),
            update_in_progress: AtomicBool::new(false)
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
        let s = Arc::clone(&state);
        thread::spawn(move || device_connection_loop(s));
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

            cmd => {
                if state.update_in_progress.load(Ordering::Relaxed) {
                    send(&mut writer, DaemonResponse::Error { 
                        message: "Firmware update in progress".to_string()
                    });
                    continue;
                }

                let mut dev = state.device.lock().unwrap();
                match dev.as_mut() {
                    None => send(&mut writer, DaemonResponse::NoDevice),
                    Some(ds) => {
                        let resp = dispatch(ds, cmd);
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

fn dispatch(ds: &mut DualSense, cmd: DaemonCommand) -> DaemonResponse {
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
            Ok(s)  => DaemonResponse::InputState(s),
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

        DaemonCommand::SetUpdateMode { .. } => unreachable!()
    }
}

