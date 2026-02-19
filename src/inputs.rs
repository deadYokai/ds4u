use serde::{Deserialize, Serialize};

pub const TOUCHPAD_MAX_X: u16 = 1920;
pub const TOUCHPAD_MAX_Y: u16 = 1080;
pub const DPAD_N:       u8 = 0;
pub const DPAD_NE:      u8 = 1;
pub const DPAD_E:       u8 = 2;
pub const DPAD_SE:      u8 = 3;
pub const DPAD_S:       u8 = 4;
pub const DPAD_SW:      u8 = 5;
pub const DPAD_W:       u8 = 6;
pub const DPAD_NW:      u8 = 7;
pub const DPAD_NEUTRAL: u8 = 8;
pub const BTN_SQUARE:   u32 = 1 << 0;
pub const BTN_CROSS:    u32 = 1 << 1;
pub const BTN_CIRCLE:   u32 = 1 << 2;
pub const BTN_TRIANGLE: u32 = 1 << 3;
pub const BTN_L1:       u32 = 1 << 4;
pub const BTN_R1:       u32 = 1 << 5;
pub const BTN_L2:       u32 = 1 << 6;
pub const BTN_R2:       u32 = 1 << 7;
pub const BTN_CREATE:   u32 = 1 << 8;
pub const BTN_OPTIONS:  u32 = 1 << 9;
pub const BTN_L3:       u32 = 1 << 10;
pub const BTN_R3:       u32 = 1 << 11;
pub const BTN_PS:       u32 = 1 << 12;
pub const BTN_TOUCHPAD: u32 = 1 << 13;
pub const BTN_MUTE:     u32 = 1 << 14;

pub struct TouchPoint {
    pub active: bool,
    pub id: u8,
    pub x: u16,
    pub y: u16
}

impl Default for TouchPoint {
    fn default() -> Self {
        Self { active: false, id: 0, x: 0, y: 0 }
    }
}

pub struct ControllerState {
    pub left_x:  u8,
    pub left_y:  u8,
    pub right_x: u8,
    pub right_y: u8,
    pub l2: u8,
    pub r2: u8,
    pub buttons: u32,
    pub dpad: u8,
    pub gyro: [i16; 3],
    pub accel: [i16; 3],
    pub sensor_timestamp: u32,
    pub touch_count: u8,
    pub touch_points: [TouchPoint; 2]
}

#[derive(Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum Button {
    Create,
    L3,
    R3,
    Options,
    DPadUp,
    DPadRight,
    DPadDown,
    DPadLeft,
    L2,
    R2,
    L1,
    R1,
    Triangle,
    Circle,
    Cross,
    Square,
    PS,
    Touchpad,
    Mute
}

impl Button {
    pub fn to_bitmask(&self) -> Option<u32> {
        match self {
            Button::Square    => Some(BTN_SQUARE),
            Button::Cross     => Some(BTN_CROSS),
            Button::Circle    => Some(BTN_CIRCLE),
            Button::Triangle  => Some(BTN_TRIANGLE),
            Button::L1        => Some(BTN_L1),
            Button::R1        => Some(BTN_R1),
            Button::L2        => Some(BTN_L2),
            Button::R2        => Some(BTN_R2),
            Button::L3        => Some(BTN_L3),
            Button::R3        => Some(BTN_R3),
            Button::PS        => Some(BTN_PS),
            Button::Create    => Some(BTN_CREATE),
            Button::Options   => Some(BTN_OPTIONS),
            Button::Touchpad  => Some(BTN_TOUCHPAD),
            Button::Mute      => Some(BTN_MUTE),
            Button::DPadUp    |
            Button::DPadDown  |
            Button::DPadLeft  |
            Button::DPadRight => None
        }
    }
}
