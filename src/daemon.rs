use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    sync::{
        Arc, Condvar, Mutex, RwLock,
        atomic::{AtomicBool, AtomicU16, Ordering},
    },
    thread::{self, sleep},
    time::{Duration, Instant},
};

use hidapi::HidApi;
use rustix::event::{PollFd, PollFlags, poll};

use crate::{
    common::{HapticPattern, LightbarEffect},
    dualsense::{
        self, DeviceDesc, DeviceInfo, DualSense, HAPTICS_PACKET_FRAMES, HAPTICS_SAMPLE_RATE,
    },
    haptics_stream::generate_packet,
    input_grab::InputGrab,
    inputs::ControllerState,
    ipc::{
        DaemonCommand, DaemonResponse, DaemonStream, IpcClient, addr_display, bind_daemon,
        cleanup_endpoint, daemon_endpoint,
    },
    profiles::ProfileManager,
    settings::SettingsManager,
    transform::{GyroProcessor, InputTransform},
    util::{mlock, rlock, wait_cv, wlock},
    virtual_gamepad::VirtualGamepad,
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

    pub fn set_update_in_progress(&mut self, device: Option<&str>, active: bool) {
        if let Some(ref arc) = self.client {
            let mut c = mlock(arc);
            if let Some(id) = device {
                let _ = c.select_device(Some(id));
            }
            let _ = c.set_update_mode(active);
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

impl DaemonInner {
    fn new() -> Self {
        Self {
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
        }
    }
}

#[derive(Clone, Copy)]
struct LatestInput {
    state: ControllerState,
    status_byte: u8,
    at: Instant,
}

struct Slot {
    id: String,
    serial: String,
    index: usize,
    product_id: AtomicU16,
    is_bt: AtomicBool,
    device: Mutex<Option<DualSense>>,
    grab: Mutex<Option<InputGrab>>,
    update_in_progress: AtomicBool,
    inner: RwLock<DaemonInner>,
    latest_input: Mutex<Option<LatestInput>>,
    stop: AtomicBool,
}

struct DaemonState {
    slots: RwLock<Vec<Arc<Slot>>>,
    hotplug: (Mutex<bool>, Condvar),
}

impl DaemonState {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            slots: RwLock::new(Vec::new()),
            hotplug: (Mutex::new(false), Condvar::new()),
        })
    }
}

fn snapshot_slots(state: &DaemonState) -> Vec<Arc<Slot>> {
    rlock(&state.slots).clone()
}

fn find_slot(state: &DaemonState, selected: &Option<String>) -> Option<Arc<Slot>> {
    let slots = rlock(&state.slots);
    match selected {
        Some(id) => slots.iter().find(|s| &s.id == id).cloned(),
        None => slots.first().cloned(),
    }
}

fn fresh_input(slot: &Slot) -> Option<LatestInput> {
    (*mlock(&slot.latest_input)).filter(|l| l.at.elapsed() < Duration::from_millis(500))
}

fn device_info(slot: &Slot) -> DeviceInfo {
    DeviceInfo {
        id: slot.id.clone(),
        serial: slot.serial.clone(),
        product_id: slot.product_id.load(Ordering::Relaxed),
        is_bt: slot.is_bt.load(Ordering::Relaxed),
        index: slot.index,
        profile: rlock(&slot.inner).active_profile_name.clone(),
        battery: fresh_input(slot).map(|l| DualSense::battery_from_status(l.status_byte)),
    }
}

fn signal_hotplug(state: &DaemonState) {
    let (l, c) = &state.hotplug;
    *mlock(l) = true;
    c.notify_all();
}

fn pad_name(index: usize) -> String {
    if index == 0 {
        "DS4U DualSense Wireless Controller".to_string()
    } else {
        format!("DS4U DualSense Wireless Controller {}", index + 1)
    }
}

fn apply_profile_to_state(slot: &Slot, name: &str) -> String {
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

    let mut inner = wlock(&slot.inner);
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

fn push_triggers_to_device(slot: &Slot) {
    let (left, right) = {
        let inner = rlock(&slot.inner);
        (inner.trigger_left, inner.trigger_right)
    };
    if let Some(ds) = mlock(&slot.device).as_mut() {
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

fn create_slot(state: &Arc<DaemonState>, desc: &DeviceDesc) -> Arc<Slot> {
    let wanted = SettingsManager::new()
        .load()
        .profile_for(&desc.id)
        .to_string();
    let wanted = if wanted.is_empty() {
        "Default".to_string()
    } else {
        wanted
    };

    let index = {
        let slots = rlock(&state.slots);
        (0..)
            .find(|i| !slots.iter().any(|s| s.index == *i))
            .unwrap_or(0)
    };

    let slot = Arc::new(Slot {
        id: desc.id.clone(),
        serial: desc.serial.clone(),
        index,
        product_id: AtomicU16::new(desc.product_id),
        is_bt: AtomicBool::new(desc.is_bt),
        device: Mutex::new(None),
        grab: Mutex::new(None),
        update_in_progress: AtomicBool::new(false),
        inner: RwLock::new(DaemonInner::new()),
        latest_input: Mutex::new(None),
        stop: AtomicBool::new(false),
    });

    let loaded = apply_profile_to_state(&slot, &wanted);
    wlock(&slot.inner).active_profile_name = loaded.clone();
    println!(
        "{} new controller {} (#{}), profile '{}'",
        TAG,
        slot.id,
        index + 1,
        loaded
    );

    wlock(&state.slots).push(Arc::clone(&slot));

    let s = Arc::clone(state);
    let sl = Arc::clone(&slot);
    thread::spawn(move || slot_worker(s, sl));

    slot
}

fn reconcile(state: &Arc<DaemonState>, api: &HidApi, found: &[DeviceDesc]) {
    {
        let mut slots = wlock(&state.slots);
        slots.retain(|s| {
            let keep =
                found.iter().any(|d| d.id == s.id) || s.update_in_progress.load(Ordering::Relaxed);
            if !keep {
                s.stop.store(true, Ordering::Relaxed);
                println!("{} controller removed: {}", TAG, s.id);
            }
            keep
        });
    }

    for desc in found {
        let existing = rlock(&state.slots)
            .iter()
            .find(|s| s.id == desc.id)
            .cloned();
        let slot = existing.unwrap_or_else(|| create_slot(state, desc));

        if slot.update_in_progress.load(Ordering::Relaxed) {
            continue;
        }

        let mut dev = mlock(&slot.device);
        if dev.is_some() {
            continue;
        }
        let Ok(mut ds) = DualSense::open_desc(api, desc) else {
            continue;
        };
        println!("{} controller connected: {}", TAG, slot.id);

        let snap = {
            let i = rlock(&slot.inner);
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

        slot.product_id.store(desc.product_id, Ordering::Relaxed);
        slot.is_bt.store(desc.is_bt, Ordering::Relaxed);
        *dev = Some(ds);
    }
}

fn device_connection_loop(state: Arc<DaemonState>) {
    loop {
        if let Ok(api) = HidApi::new() {
            let found = dualsense::discover(&api);
            reconcile(&state, &api, &found);
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

    println!("{} udev hotplug watcher active", TAG);

    loop {
        let mut fds = [PollFd::new(&socket, PollFlags::IN)];
        if poll(&mut fds, None).is_err() {
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
            signal_hotplug(&state);
        }
    }
}

enum Poll {
    NoDevice,
    Failed,
    Data(
        (u16, u16),
        Option<crate::dualsense::PolledInput>,
        Option<String>,
    ),
}

fn slot_worker(state: Arc<DaemonState>, slot: Arc<Slot>) {
    let mut pad: Option<VirtualGamepad> = None;
    let mut pad_ids: Option<(u16, u16)> = None;
    let mut pad_disabled = false;
    let mut grabbed = false;

    while !slot.stop.load(Ordering::Relaxed) {
        sleep(Duration::from_millis(1));
        if slot.update_in_progress.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(20));
            continue;
        }

        let want_path = !grabbed;
        let poll = {
            let mut dev = mlock(&slot.device);
            match dev.as_mut() {
                None => Poll::NoDevice,
                Some(ds) => {
                    let ids = (ds.vendor_id(), ds.product_id());
                    match ds.poll_latest_input() {
                        Ok(p) => Poll::Data(ids, p, want_path.then(|| ds.hid_path().to_string())),
                        Err(_) => Poll::Failed,
                    }
                }
            }
        };

        let (ids, polled, hid_path) = match poll {
            Poll::NoDevice => {
                *mlock(&slot.grab) = None;
                grabbed = false;
                sleep(Duration::from_millis(20));
                continue;
            }
            Poll::Failed => {
                println!("{} {}: input read failed - dropping handle", TAG, slot.id);
                *mlock(&slot.device) = None;
                *mlock(&slot.grab) = None;
                grabbed = false;
                signal_hotplug(&state);
                continue;
            }
            Poll::Data(ids, p, path) => (ids, p, path),
        };

        if !pad_disabled && pad_ids != Some(ids) {
            pad = None;
            match VirtualGamepad::new(ids.0, ids.1, &pad_name(slot.index)) {
                Ok(p) => {
                    println!(
                        "{} {}: virtual gamepad {:04x}:{:04x}",
                        TAG, slot.id, ids.0, ids.1
                    );
                    pad = Some(p);
                    pad_ids = Some(ids);
                }
                Err(e) => {
                    eprintln!("{} virtual gamepad disabled: {}", TAG, e);
                    pad_disabled = true;
                }
            }
        }

        if pad.is_some()
            && !grabbed
            && let Some(path) = hid_path
        {
            sleep(Duration::from_millis(200));
            *mlock(&slot.grab) = None;
            *mlock(&slot.grab) = Some(InputGrab::acquire_for(&path));
            grabbed = true;
        }

        let Some(p) = polled else { continue };

        *mlock(&slot.latest_input) = Some(LatestInput {
            state: p.state,
            status_byte: p.status_byte,
            at: Instant::now(),
        });

        if let Some(pad) = pad.as_mut() {
            let mut st = p.state;
            rlock(&slot.inner).active_transform.apply(&mut st);
            let _ = pad.update(&st);
        }
    }

    *mlock(&slot.grab) = None;
}

fn no_input_resp(slot: &Slot) -> DaemonResponse {
    if mlock(&slot.device).is_some() {
        DaemonResponse::Error {
            message: "no fresh input report".into(),
        }
    } else {
        DaemonResponse::NoDevice
    }
}

fn handle_client(stream: DaemonStream, state: Arc<DaemonState>) {
    let write_half = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };

    let mut reader = BufReader::new(stream);
    let mut writer = write_half;

    let mut selected: Option<String> = None;

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

        let slot = find_slot(&state, &selected);
        macro_rules! need_slot {
            () => {
                match slot.clone() {
                    Some(s) => s,
                    None => {
                        send(&mut writer, DaemonResponse::NoDevice);
                        continue;
                    }
                }
            };
        }

        match cmd {
            DaemonCommand::Ping => {
                send(&mut writer, DaemonResponse::Pong);
            }

            DaemonCommand::Shutdown => {
                send(&mut writer, DaemonResponse::Ok);
                println!("{} shutdown requested via IPC", TAG);
                std::process::exit(0);
            }

            DaemonCommand::ListDevices => {
                let devices = snapshot_slots(&state)
                    .iter()
                    .map(|s| device_info(s))
                    .collect();
                send(&mut writer, DaemonResponse::DeviceList { devices });
            }

            DaemonCommand::SelectDevice { id } => match id {
                None => {
                    selected = None;
                    send(&mut writer, DaemonResponse::Ok);
                }
                Some(id) => {
                    if rlock(&state.slots).iter().any(|s| s.id == id) {
                        selected = Some(id);
                        send(&mut writer, DaemonResponse::Ok);
                    } else {
                        send(&mut writer, DaemonResponse::NoDevice);
                    }
                }
            },

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
                let name = match &slot {
                    Some(s) => rlock(&s.inner).active_profile_name.clone(),
                    None => SettingsManager::new().load().profile,
                };
                send(&mut writer, DaemonResponse::ActiveProfile { name });
            }

            DaemonCommand::SwitchProfile { name } => {
                let sm = SettingsManager::new();
                let mut settings = sm.load();
                match slot {
                    Some(slot) => {
                        let loaded = apply_profile_to_state(&slot, &name);
                        wlock(&slot.inner).active_profile_name = loaded.clone();
                        settings.set_profile_for(&slot.id, &loaded);
                        sm.save(&settings);

                        let (color, leds, mic) = {
                            let i = rlock(&slot.inner);
                            (i.lightbar_color, i.player_leds, i.mic_enabled)
                        };
                        let (r, g, b, br) = color;
                        if let Some(ds) = mlock(&slot.device).as_mut() {
                            let _ = ds.set_lightbar(r, g, b, br);
                            let _ = ds.set_player_leds(leds);
                            let _ = ds.set_mic(mic);
                        }
                        push_triggers_to_device(&slot);
                        println!("{} {}: switched to profile '{}'", TAG, slot.id, loaded);
                    }
                    None => {
                        settings.profile = name;
                        sm.save(&settings);
                    }
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ReloadProfile => {
                let slot = need_slot!();
                let name = rlock(&slot.inner).active_profile_name.clone();
                let name = if name.is_empty() {
                    "Default".to_string()
                } else {
                    name
                };
                apply_profile_to_state(&slot, &name);
                push_triggers_to_device(&slot);
                println!("{} {}: reloaded profile '{}'", TAG, slot.id, name);
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetUpdateMode { active } => {
                let slot = need_slot!();
                if active {
                    slot.update_in_progress.store(true, Ordering::SeqCst);
                    *mlock(&slot.device) = None;
                    *mlock(&slot.grab) = None;
                    println!("{} {}: released for firmware update", TAG, slot.id);
                } else {
                    slot.update_in_progress.store(false, Ordering::SeqCst);
                    println!(
                        "{} {}: firmware update done, device will reconnect",
                        TAG, slot.id
                    );
                    signal_hotplug(&state);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetInputTransform { transform } => {
                let slot = need_slot!();
                wlock(&slot.inner).active_transform = transform;
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::ClearInputTransform => {
                let slot = need_slot!();
                wlock(&slot.inner).active_transform = InputTransform::default();
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetLightbarEffect { effect } => {
                let slot = need_slot!();
                let restoring = matches!(effect, LightbarEffect::None);
                wlock(&slot.inner).active_effect = effect;

                if restoring {
                    let (r, g, b, br) = rlock(&slot.inner).lightbar_color;
                    if let Some(ds) = mlock(&slot.device).as_mut() {
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
                let slot = need_slot!();
                let restoring = matches!(pattern, HapticPattern::None);
                wlock(&slot.inner).haptic = (pattern, strength.min(7), speed.max(0.05));
                if restoring && let Some(ds) = mlock(&slot.device).as_mut() {
                    let _ = ds.set_vibration(0, 0);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetRawHaptics { active } => {
                let slot = need_slot!();
                wlock(&slot.inner).raw_haptics = active;
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetGyro {
                enabled,
                smoothing,
                sensitivity,
            } => {
                let slot = need_slot!();
                {
                    let mut inner = wlock(&slot.inner);
                    let g = &mut inner.gyro;
                    g.enabled = enabled;
                    g.smoothing = smoothing.clamp(0.0, 0.95);
                    g.sensitivity = sensitivity.max(0.0);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::SetTriggerEffects { left, right } => {
                let slot = need_slot!();
                {
                    let mut inner = wlock(&slot.inner);
                    if let Some(l) = left {
                        inner.trigger_left = Some(l);
                    }
                    if let Some(r) = right {
                        inner.trigger_right = Some(r);
                    }
                }
                if slot.update_in_progress.load(Ordering::Relaxed) {
                    send(
                        &mut writer,
                        DaemonResponse::Error {
                            message: "Firmware update in progress".to_string(),
                        },
                    );
                    continue;
                }
                let (l, r) = {
                    let i = rlock(&slot.inner);
                    (i.trigger_left, i.trigger_right)
                };
                if let Some(ds) = mlock(&slot.device).as_mut() {
                    let lf = l.or(Some((0x05, [0u8; 10])));
                    let rt = r.or(Some((0x05, [0u8; 10])));
                    let _ = ds.set_trigger_effects(lf, rt);
                }
                send(&mut writer, DaemonResponse::Ok);
            }

            DaemonCommand::GetInputState => {
                let slot = need_slot!();
                match fresh_input(&slot) {
                    Some(l) => {
                        let mut s = l.state;
                        rlock(&slot.inner).active_transform.apply(&mut s);
                        send(&mut writer, DaemonResponse::InputState(s));
                    }
                    None => send(&mut writer, no_input_resp(&slot)),
                }
            }

            DaemonCommand::GetBattery => {
                let slot = need_slot!();
                match fresh_input(&slot) {
                    Some(l) => send(
                        &mut writer,
                        DaemonResponse::Battery(DualSense::battery_from_status(l.status_byte)),
                    ),
                    None => send(&mut writer, no_input_resp(&slot)),
                }
            }

            cmd @ (DaemonCommand::GetFirmwareInfo | DaemonCommand::GetControllerInfo) => {
                let slot = need_slot!();
                let transform = rlock(&slot.inner).active_transform.clone();
                let mut dev = mlock(&slot.device);
                match dev.as_mut() {
                    None => send(&mut writer, DaemonResponse::NoDevice),
                    Some(ds) => {
                        let resp = dispatch(ds, cmd, &transform);
                        send(&mut writer, resp);
                    }
                }
            }

            cmd => {
                let slot = need_slot!();
                if slot.update_in_progress.load(Ordering::Relaxed) {
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
                    let mut inner = wlock(&slot.inner);
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
                    let mut inner = wlock(&slot.inner);
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
                    let mut inner = wlock(&slot.inner);
                    inner.trigger_left = None;
                    inner.trigger_right = None;
                }

                let transform = rlock(&slot.inner).active_transform.clone();
                let mut dev = mlock(&slot.device);
                match dev.as_mut() {
                    None => {
                        signal_hotplug(&state);
                        send(&mut writer, DaemonResponse::NoDevice);
                    }
                    Some(ds) => {
                        let resp = dispatch(ds, cmd, &transform);
                        let failed = matches!(&resp, DaemonResponse::Error { .. });
                        send(&mut writer, resp);
                        if failed {
                            println!("{} {}: device error - dropping handle", TAG, slot.id);
                            *dev = None;
                            signal_hotplug(&state);
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
        let t = start.elapsed().as_secs_f32();

        for slot in snapshot_slots(&state) {
            let effect = rlock(&slot.inner).active_effect.clone();
            if matches!(effect, LightbarEffect::None) {
                continue;
            }
            if slot.update_in_progress.load(Ordering::Relaxed) {
                continue;
            }

            let (base_r, base_g, base_b, base_br) = rlock(&slot.inner).lightbar_color;

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

            if let Ok(mut dev) = slot.device.try_lock()
                && let Some(ds) = dev.as_mut()
            {
                let _ = ds.set_lightbar(r, g, b, base_br);
            }
        }
    }
}

fn haptic_loop(state: Arc<DaemonState>) {
    let start = Instant::now();
    let mut last_amp: HashMap<String, u8> = HashMap::new();

    loop {
        sleep(Duration::from_millis(40));
        let t = start.elapsed().as_secs_f32();

        for slot in snapshot_slots(&state) {
            if slot.update_in_progress.load(Ordering::Relaxed) {
                continue;
            }
            let last = last_amp.entry(slot.id.clone()).or_insert(255);

            let (raw, (pattern, strength, speed)) = {
                let g = rlock(&slot.inner);
                (g.raw_haptics, g.haptic)
            };

            if raw || matches!(pattern, HapticPattern::None) {
                if *last != 0 {
                    if let Some(ds) = mlock(&slot.device).as_mut() {
                        let _ = ds.set_rumble(0, 0);
                    }
                    *last = 0;
                }
                continue;
            }

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
            if amp != *last {
                if let Some(ds) = mlock(&slot.device).as_mut() {
                    let _ = ds.set_rumble(amp, amp);
                }
                *last = amp;
            }
        }
    }
}

fn raw_haptic_loop(state: Arc<DaemonState>) {
    let start = Instant::now();
    let period = Duration::from_secs_f32(HAPTICS_PACKET_FRAMES as f32 / HAPTICS_SAMPLE_RATE as f32);
    loop {
        sleep(period);
        let t0 = start.elapsed().as_secs_f32();

        for slot in snapshot_slots(&state) {
            if slot.update_in_progress.load(Ordering::Relaxed) {
                continue;
            }
            let (raw, (pattern, strength, speed)) = {
                let g = rlock(&slot.inner);
                (g.raw_haptics, g.haptic)
            };
            if !raw || matches!(pattern, HapticPattern::None) {
                continue;
            }
            let packet = generate_packet(pattern, strength, speed, t0);
            let mut dev = mlock(&slot.device);
            if let Some(ds) = dev.as_mut().filter(|d| d.is_bluetooth()) {
                let _ = ds.set_haptics(&packet);
            }
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
