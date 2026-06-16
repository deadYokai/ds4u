use serde::{Deserialize, Serialize};

pub const DS_VID: u16 = 0x054c;
pub const DS_PID: u16 = 0x0ce6;
pub const DSE_PID: u16 = 0x0df2;
pub const FIRMWARE_SIZE: usize = 950272;

pub const DS_TRIGGER_EFFECT_OFF: u8 = 0x05;
pub const DS_TRIGGER_EFFECT_FEEDBACK: u8 = 0x21;
pub const DS_TRIGGER_EFFECT_BOW: u8 = 0x22;
pub const DS_TRIGGER_EFFECT_GALLOPING: u8 = 0x23;
pub const DS_TRIGGER_EFFECT_WEAPON: u8 = 0x25;
pub const DS_TRIGGER_EFFECT_VIBRATION: u8 = 0x26;
pub const DS_TRIGGER_EFFECT_MACHINE: u8 = 0x27;

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Debug, Default)]
pub enum TouchpadMode {
    Disabled,
    #[default]
    Mouse,
    GesturesOnly,
    PassThrough,
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum MicLedState {
    Off,
    On,
    Pulse,
}

#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub enum TriggerMode {
    Off,
    Feedback,
    Weapon,
    Bow,
    Galloping,
    Vibration,
    Machine,
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub enum SensitivityCurve {
    Default,
    Quick,
    Precise,
    Steady,
    Digital,
    Dynamic,
}

#[derive(PartialEq)]
pub enum SpeakerMode {
    Internal,
    Headphone,
    Both,
}

#[derive(Clone, PartialEq, Serialize, Deserialize, Default)]
pub enum LightbarEffect {
    #[default]
    None,
    Breath {
        speed: f32,
    },
    Rainbow {
        speed: f32,
    },
    Strobe {
        speed: f32,
    },
}

#[derive(Clone, Copy, PartialEq, Serialize, Deserialize, Debug, Default)]
pub enum HapticPattern {
    #[default]
    None,
    Constant,
    Pulse,
    Ramp,
    Wave,
}
