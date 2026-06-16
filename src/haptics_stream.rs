use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::common::HapticPattern;
use crate::dualsense::{
    DualSense, HAPTICS_PACKET_FRAMES, HAPTICS_PACKET_SAMPLES, HAPTICS_SAMPLE_RATE,
};
use crate::util::mlock;

pub const HAPTIC_CARRIER_HZ: f32 = 160.0;

type Params = (HapticPattern, u8, f32);

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

pub fn generate_packet(
    pattern: HapticPattern,
    strength: u8,
    speed: f32,
    t0: f32,
) -> [i8; HAPTICS_PACKET_SAMPLES] {
    let mut out = [0i8; HAPTICS_PACKET_SAMPLES];
    let dt = 1.0 / HAPTICS_SAMPLE_RATE as f32;
    for frame in 0..HAPTICS_PACKET_FRAMES {
        let t = t0 + frame as f32 * dt;
        let env = envelope(pattern, strength, speed, t).clamp(0.0, 1.0);
        let carrier = (t * HAPTIC_CARRIER_HZ * std::f32::consts::TAU).sin();
        let v = (carrier * env * 127.0).round().clamp(-127.0, 127.0) as i8;
        out[frame * 2] = v;
        out[frame * 2 + 1] = v;
    }
    out
}

pub struct HapticStream {
    stop: Option<Arc<AtomicBool>>,
    thread: Option<JoinHandle<()>>,
    params: Arc<Mutex<Params>>,
}

impl HapticStream {
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

    pub fn start(
        &mut self,
        ctrl: Arc<Mutex<DualSense>>,
        pattern: HapticPattern,
        strength: u8,
        speed: f32,
    ) {
        self.stop();

        *mlock(&self.params) = (pattern, strength, speed);
        let params = Arc::clone(&self.params);

        let stop = Arc::new(AtomicBool::new(false));
        let stop_c = Arc::clone(&stop);

        let period =
            Duration::from_secs_f32(HAPTICS_PACKET_FRAMES as f32 / HAPTICS_SAMPLE_RATE as f32);
        let start = Instant::now();

        let handle = thread::spawn(move || {
            let mut next = Instant::now();
            while !stop_c.load(Ordering::Relaxed) {
                let (pattern, strength, speed) = *mlock(&params);
                let t0 = start.elapsed().as_secs_f32();
                let packet = generate_packet(pattern, strength, speed, t0);
                {
                    let mut dev = mlock(&ctrl);
                    if dev.set_haptics(&packet).is_err() {
                        break;
                    }
                }
                next += period;
                let now = Instant::now();
                if next > now {
                    thread::sleep(next - now);
                } else {
                    next = now;
                }
            }
            let silence = [0i8; HAPTICS_PACKET_SAMPLES];
            let _ = mlock(&ctrl).set_haptics(&silence);
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

impl Default for HapticStream {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for HapticStream {
    fn drop(&mut self) {
        self.stop();
    }
}
