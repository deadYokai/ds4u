use std::sync::{Arc, Mutex};

use crate::{
    common::{HapticPattern, LightbarEffect, MicLedState},
    dualsense::DualSense,
    ipc::IpcClient,
    transform::InputTransform,
    util::mlock,
};

pub(crate) const TRIGGER_OFF: (u8, [u8; 10]) = (0x05, [0u8; 10]);

pub(crate) trait ControllerBackend {
    fn set_lightbar(&self, r: u8, g: u8, b: u8, brightness: u8);
    fn set_player_leds(&self, leds: u8);
    fn set_mic(&self, enabled: bool);
    fn set_mic_led(&self, state: MicLedState);
    fn set_vibration(&self, rumble: u8, trigger: u8);
    fn set_trigger_effects(&self, left: Option<(u8, [u8; 10])>, right: Option<(u8, [u8; 10])>);

    fn set_lightbar_effect(&self, _effect: LightbarEffect) {}
    fn set_haptic_pattern(&self, _pattern: HapticPattern, _strength: u8, _speed: f32) {}
    fn set_gyro(&self, _enabled: bool, _smoothing: f32, _sensitivity: f32) {}
    fn set_input_transform(&self, _transform: InputTransform) {}
}

pub(crate) struct DirectBackend(pub Arc<Mutex<DualSense>>);

pub(crate) struct IpcBackend(pub Arc<Mutex<IpcClient>>);

impl ControllerBackend for DirectBackend {
    fn set_lightbar(&self, r: u8, g: u8, b: u8, brightness: u8) {
        let _ = mlock(&self.0).set_lightbar(r, g, b, brightness);
    }

    fn set_player_leds(&self, leds: u8) {
        let _ = mlock(&self.0).set_player_leds(leds);
    }

    fn set_mic(&self, enabled: bool) {
        let _ = mlock(&self.0).set_mic(enabled);
    }

    fn set_mic_led(&self, state: MicLedState) {
        let _ = mlock(&self.0).set_mic_led(state);
    }

    fn set_vibration(&self, rumble: u8, trigger: u8) {
        let _ = mlock(&self.0).set_vibration(rumble, trigger);
    }

    fn set_trigger_effects(&self, left: Option<(u8, [u8; 10])>, right: Option<(u8, [u8; 10])>) {
        let _ = mlock(&self.0).set_trigger_effects(left, right);
    }
}

impl ControllerBackend for IpcBackend {
    fn set_lightbar(&self, r: u8, g: u8, b: u8, brightness: u8) {
        let _ = mlock(&self.0).set_lightbar(r, g, b, brightness);
    }

    fn set_player_leds(&self, leds: u8) {
        let _ = mlock(&self.0).set_player_leds(leds);
    }

    fn set_mic(&self, enabled: bool) {
        let _ = mlock(&self.0).set_mic(enabled);
    }

    fn set_mic_led(&self, state: MicLedState) {
        let _ = mlock(&self.0).set_mic_led(state);
    }

    fn set_vibration(&self, rumble: u8, trigger: u8) {
        let _ = mlock(&self.0).set_vibration(rumble, trigger);
    }

    fn set_trigger_effects(&self, left: Option<(u8, [u8; 10])>, right: Option<(u8, [u8; 10])>) {
        let _ = mlock(&self.0).set_trigger_effects(left, right);
    }

    fn set_lightbar_effect(&self, effect: LightbarEffect) {
        let _ = mlock(&self.0).set_lightbar_effect(effect);
    }

    fn set_haptic_pattern(&self, pattern: HapticPattern, strength: u8, speed: f32) {
        let _ = mlock(&self.0).set_haptic_pattern(pattern, strength, speed);
    }

    fn set_gyro(&self, enabled: bool, smoothing: f32, sensitivity: f32) {
        let _ = mlock(&self.0).set_gyro(enabled, smoothing, sensitivity);
    }

    fn set_input_transform(&self, transform: InputTransform) {
        let _ = mlock(&self.0).set_input_transform(transform);
    }
}
