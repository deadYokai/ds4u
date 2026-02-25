use crate::common::{MicLedState, TriggerMode, SensitivityCurve, SpeakerMode};

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
    Lightbar, Triggers, Sticks, Haptics, Audio, Advanced, Inputs,
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

pub(crate) struct TriggerState {
    pub(crate) mode: TriggerMode,
    pub(crate) position: u8,
    pub(crate) strength: u8,
}

pub(crate) struct StickSettings {
    pub(crate) left_curve: SensitivityCurve,
    pub(crate) right_curve: SensitivityCurve,
    pub(crate) left_deadzone: f32,
    pub(crate) right_deadzone: f32,
}

pub(crate) struct AudioSettings {
    pub(crate) volume: u8,
    pub(crate) speaker_mode: SpeakerMode,
}

pub(crate) struct VibrationSettings {
    pub(crate) rumble: u8,
    pub(crate) trigger: u8,
}
