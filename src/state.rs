use crate::common::{HapticPattern, MicLedState, SpeakerMode};
use crate::profiles::TriggerConfig;
use crate::transform::GyroProcessor;

#[derive(Debug, Clone)]
pub(crate) enum ProgressUpdate {
    Progress(u32),
    Status(String),
    Complete,
    Error(String),
    LatestVersion(String),
}

#[derive(PartialEq)]
pub(crate) enum Section {
    Lightbar,
    Triggers,
    Sticks,
    Haptics,
    Audio,
    Advanced,
    Inputs,
    Settings,
    Gyroscope,
    Touchpad,
    Profiles,
}

pub(crate) struct LightbarState {
    pub(crate) r: f32,
    pub(crate) g: f32,
    pub(crate) b: f32,
    pub(crate) brightness: f32,
    pub(crate) enabled: bool,
}

pub(crate) struct MicrophoneState {
    pub(crate) enabled: bool,
    pub(crate) led_state: MicLedState,
}

pub(crate) struct StickSettings {
    pub(crate) left_curve: crate::common::SensitivityCurve,
    pub(crate) right_curve: crate::common::SensitivityCurve,
    pub(crate) left_deadzone: f32,
    pub(crate) right_deadzone: f32,
    pub(crate) left_outer_deadzone: f32,
    pub(crate) right_outer_deadzone: f32,
    pub(crate) left_invert_x: bool,
    pub(crate) left_invert_y: bool,
    pub(crate) right_invert_x: bool,
    pub(crate) right_invert_y: bool,
    pub(crate) swap: bool,
}

pub(crate) struct AudioSettings {
    pub(crate) volume: u8,
    pub(crate) speaker_mode: SpeakerMode,
}

pub(crate) struct VibrationSettings {
    pub(crate) rumble: u8,
    pub(crate) trigger: u8,
}
pub(crate) struct GyroState {
    pub(crate) processor: GyroProcessor,
}

pub(crate) struct TouchpadState {
    pub(crate) enabled: bool,
    pub(crate) show_overlay: bool,
}

pub(crate) struct HapticState {
    pub(crate) pattern: HapticPattern,
    pub(crate) strength: u8,
    pub(crate) speed: f32,
}

pub(crate) struct TriggersState {
    pub(crate) left: TriggerConfig,
    pub(crate) right: TriggerConfig,
}
