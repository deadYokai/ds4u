use serde::{Deserialize, Serialize};

pub const DS_VID: u16 = 0x054c;
pub const DS_PID: u16 = 0x0ce6;
pub const DSE_PID: u16 = 0x0df2;
pub const FIRMWARE_SIZE: usize = 950272;

pub const BTN_SQUARE: u32 = 1 << 0;
pub const BTN_CROSS: u32 = 1 << 1;
pub const BTN_CIRCLE: u32 = 1 << 2;
pub const BTN_TRIANGLE: u32 = 1 << 3;
pub const BTN_L1: u32 = 1 << 4;
pub const BTN_R1: u32 = 1 << 5;
pub const BTN_L2: u32 = 1 << 6;
pub const BTN_R2: u32 = 1 << 7;
pub const BTN_CREATE: u32 = 1 << 8;
pub const BTN_OPTIONS: u32 = 1 << 9;
pub const BTN_L3: u32 = 1 << 10;
pub const BTN_R3: u32 = 1 << 11;
pub const BTN_PS: u32 = 1 << 12;
pub const BTN_TOUCHPAD: u32 = 1 << 13;
pub const BTN_MUTE: u32 = 1 << 14;

pub const DPAD_NEUTRAL: u8 = 8;
pub const DPAD_N: u8 = 0;
pub const DPAD_NE: u8 = 1;
pub const DPAD_E: u8 = 2;
pub const DPAD_SE: u8 = 3;
pub const DPAD_S: u8 = 4;
pub const DPAD_SW: u8 = 5;
pub const DPAD_W: u8 = 6;
pub const DPAD_NW: u8 = 7;

pub const TOUCHPAD_MAX_X: u16 = 1920;
pub const TOUCHPAD_MAX_Y: u16 = 1080;

#[derive(Deserialize, Serialize, PartialEq, Clone)]
pub enum TriggerMode {
    Off,
    Feedback,
    Weapon,
    Bow,
    Galloping,
    Vibration,
    Machine
}

#[derive(Deserialize, Serialize, Clone, PartialEq, Debug)]
pub enum SensitivityCurve {
    Default,
    Quick,
    Precise,
    Steady,
    Digital,
    Dynamic
}

#[derive(PartialEq)]
pub enum SpeakerMode {
    Internal,
    Headphone,
    Both
}

#[derive(Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Button {
    Square,
    Cross,
    Circle,
    Triangle,
    L1,
    R1,
    L2,
    R2,
    Create,
    Options,
    L3,
    R3,
    PS,
    Touchpad,
    Mute,
    DPadUp,
    DPadDown,
    DPadLeft,
    DPadRight
}

