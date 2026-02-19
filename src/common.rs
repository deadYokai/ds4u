use serde::{Deserialize, Serialize};

pub const DS_VID: u16 = 0x054c;
pub const DS_PID: u16 = 0x0ce6;
pub const DSE_PID: u16 = 0x0df2;
pub const FIRMWARE_SIZE: usize = 950272;

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

