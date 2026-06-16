use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use alsa::pcm::{Access, Format, HwParams, PCM};
use alsa::{Direction, ValueOr};
use anyhow::{Context, Result};

use crate::common::{DS_PID, DS_VID, DSE_PID, HapticPattern};
use crate::util::mlock;

const USB_AUDIO_CHANNELS: usize = 4;
const USB_AUDIO_RATE: u32 = 48_000;
const HAPTIC_L_CHANNEL: usize = 2;
const HAPTIC_R_CHANNEL: usize = 3;
const WRITE_FRAMES: usize = 480;
pub const HAPTIC_CARRIER_HZ: f32 = 160.0;

type Params = (HapticPattern, u8, f32);

pub fn find_dualsense_card() -> Option<u32> {
    let want_ds = format!("{:04x}:{:04x}", DS_VID, DS_PID);
    let want_dse = format!("{:04x}:{:04x}", DS_VID, DSE_PID);
    for n in 0..32u32 {
        let path = format!("/proc/asound/card{n}/usbid");
        if let Ok(id) = std::fs::read_to_string(&path) {
            let id = id.trim().to_ascii_lowercase();
            if id == want_ds || id == want_dse {
                return Some(n);
            }
        }
    }
    None
}

fn envelope(pattern: HapticPattern, strength: u8, speed: f32, t: f32) -> f32 {
    let s = (strength.min(7) as f32 / 7.0).clamp(0.0, 1.0);
    match pattern {
        HapticPattern::None => 0.0,
        HapticPattern::Constant => s,
        HapticPattern::Pulse => {
            let phase = (t * speed).rem_euclid(1.0);
            if phase < 0.5 { s } else { 0.0 }
        }
        HapticPattern::Ramp => {
            let phase = (t * speed).rem_euclid(1.0);
            s * phase
        }
        HapticPattern::Wave => {
            let v = (t * speed * std::f32::consts::TAU).sin() * 0.5 + 0.5;
            s * v
        }
    }
}

fn open_pcm(card: u32) -> Result<PCM> {
    let name = format!("hw:{card},0");
    let pcm = PCM::new(&name, Direction::Playback, false)
        .with_context(|| format!("opening ALSA device {name}"))?;
    {
        let hwp = HwParams::any(&pcm)?;
        hwp.set_access(Access::RWInterleaved)?;
        hwp.set_format(Format::S16LE)?;
        hwp.set_channels(USB_AUDIO_CHANNELS as u32)?;
        hwp.set_rate(USB_AUDIO_RATE, ValueOr::Nearest)?;
        pcm.hw_params(&hwp)?;
    }
    Ok(pcm)
}

fn run_stream(pcm: &PCM, stop: &AtomicBool, params: &Arc<Mutex<Params>>) -> Result<()> {
    let io = pcm.io_i16()?;
    pcm.prepare()?;

    let dt = 1.0 / USB_AUDIO_RATE as f32;
    let mut t: f32 = 0.0;
    let mut buf = vec![0i16; WRITE_FRAMES * USB_AUDIO_CHANNELS];

    while !stop.load(Ordering::Relaxed) {
        // Read the latest parameters each buffer so UI changes take effect live.
        let (pattern, strength, speed) = *mlock(params);
        for frame in 0..WRITE_FRAMES {
            let env = envelope(pattern, strength, speed, t).clamp(0.0, 1.0);
            let carrier = (t * HAPTIC_CARRIER_HZ * std::f32::consts::TAU).sin();
            let v = (carrier * env * 32767.0).round().clamp(-32767.0, 32767.0) as i16;
            let base = frame * USB_AUDIO_CHANNELS;
            buf[base] = 0; // speaker L
            buf[base + 1] = 0; // speaker R
            buf[base + HAPTIC_L_CHANNEL] = v;
            buf[base + HAPTIC_R_CHANNEL] = v;
            t += dt;
        }
        if let Err(e) = io.writei(&buf) {
            pcm.try_recover(e, true)?;
        }
    }

    for x in buf.iter_mut() {
        *x = 0;
    }
    let _ = io.writei(&buf);
    Ok(())
}

pub struct UsbHapticStream {
    stop: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
    params: Arc<Mutex<Params>>,
}

impl UsbHapticStream {
    pub fn new() -> Self {
        Self {
            stop: None,
            thread: None,
            params: Arc::new(Mutex::new((HapticPattern::None, 0, 1.0))),
        }
    }

    pub fn is_active(&self) -> bool {
        self.thread.as_ref().is_some_and(|h| !h.is_finished())
    }

    pub fn set_params(&self, pattern: HapticPattern, strength: u8, speed: f32) {
        *mlock(&self.params) = (pattern, strength, speed);
    }

    pub fn start(&mut self, card: u32, pattern: HapticPattern, strength: u8, speed: f32) {
        self.stop();

        *mlock(&self.params) = (pattern, strength, speed);
        let params = Arc::clone(&self.params);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);

        let handle = thread::spawn(move || {
            let pcm = match open_pcm(card) {
                Ok(pcm) => pcm,
                Err(e) => {
                    eprintln!("[usb-haptics] open failed: {e}");
                    return;
                }
            };
            if let Err(e) = run_stream(&pcm, &stop_c, &params) {
                eprintln!("[usb-haptics] stream ended: {e}");
            }
        });

        self.stop = Some(stop);
        self.thread = Some(handle);
    }

    pub fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
        if let Some(handle) = self.thread.take() {
            let _ = handle.join();
        }
    }
}

impl Default for UsbHapticStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for UsbHapticStream {
    fn drop(&mut self) {
        self.stop();
    }
}
