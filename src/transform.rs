
use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

use crate::{common::SensitivityCurve, inputs::*};

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct TriggerDeadband {
    pub release:     u8,
    pub full_stroke: u8,
}

impl Default for TriggerDeadband {
    fn default() -> Self {
        Self { release: 0, full_stroke: 255 }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct InputTransform {
    pub left_curve:       SensitivityCurve,
    pub right_curve:      SensitivityCurve,
    pub left_deadzone:    f32,
    pub right_deadzone:   f32,
    pub trigger_left:     TriggerDeadband,
    pub trigger_right:    TriggerDeadband,
    pub button_remap:     HashMap<Button, Button>,
    pub disabled_buttons: HashSet<Button>,
}

impl Default for InputTransform {
    fn default() -> Self {
        Self {
            left_curve:       SensitivityCurve::Default,
            right_curve:      SensitivityCurve::Default,
            left_deadzone:    0.0,
            right_deadzone:   0.0,
            trigger_left:     TriggerDeadband::default(),
            trigger_right:    TriggerDeadband::default(),
            button_remap:     HashMap::new(),
            disabled_buttons: HashSet::new(),
        }
    }
}

impl InputTransform {
    pub fn apply(&self, s: &mut ControllerState) {
        apply_stick(
            s.left_x, s.left_y, self.left_deadzone, &self.left_curve,
            &mut s.left_x, &mut s.left_y,
        );
        apply_stick(
            s.right_x, s.right_y, self.right_deadzone, &self.right_curve,
            &mut s.right_x, &mut s.right_y,
        );
        s.l2 = apply_trigger(s.l2, &self.trigger_left);
        s.r2 = apply_trigger(s.r2, &self.trigger_right);
        if !self.button_remap.is_empty() || !self.disabled_buttons.is_empty() {
            let (b, d) = remap_buttons(
                s.buttons, s.dpad, &self.button_remap, &self.disabled_buttons,
            );
            s.buttons = b;
            s.dpad    = d;
        }
    }
}

fn curve_apply(t: f32, curve: &SensitivityCurve) -> f32 {
    match curve {
        SensitivityCurve::Default => t,
        SensitivityCurve::Quick   => t.powf(0.5),
        SensitivityCurve::Precise => t.powf(2.2),
        SensitivityCurve::Steady  => t.powf(1.6),
        SensitivityCurve::Digital => if t > 0.5 { 1.0 } else { 0.0 },
        SensitivityCurve::Dynamic => {
            let t2 = t * 2.0;
            if t < 0.5 { 0.5 * t2 * t2 }
            else       { 1.0 - 0.5 * (2.0 - t2) * (2.0 - t2) }
        }
    }
}

fn apply_stick(
    raw_x: u8, raw_y: u8,
    deadzone: f32, curve: &SensitivityCurve,
    out_x: &mut u8, out_y: &mut u8,
) {
    let nx = (raw_x as f32 - 128.0) / 127.0;
    let ny = (raw_y as f32 - 128.0) / 127.0;
    let magnitude = (nx * nx + ny * ny).sqrt().min(1.0);

    if magnitude <= deadzone {
        *out_x = 128;
        *out_y = 128;
        return;
    }

    let scaled = (magnitude - deadzone) / (1.0 - deadzone).max(f32::EPSILON);
    let curved = curve_apply(scaled, curve);
    let factor = curved / magnitude;

    *out_x = (nx * factor * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8;
    *out_y = (ny * factor * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8;
}

fn apply_trigger(raw: u8, db: &TriggerDeadband) -> u8 {
    if *db == TriggerDeadband::default() { return raw; }
    let full = db.full_stroke.max(db.release.saturating_add(1));
    if raw <= db.release { return 0; }
    if raw >= full       { return 255; }
    ((raw - db.release) as f32 / (full - db.release) as f32 * 255.0).round() as u8
}

fn dpad_to_dirs(dpad: u8) -> [bool; 4] {
    match dpad {
        DPAD_N  => [true,  false, false, false],
        DPAD_NE => [true,  true,  false, false],
        DPAD_E  => [false, true,  false, false],
        DPAD_SE => [false, true,  true,  false],
        DPAD_S  => [false, false, true,  false],
        DPAD_SW => [false, false, true,  true ],
        DPAD_W  => [false, false, false, true ],
        DPAD_NW => [true,  false, false, true ],
        _       => [false, false, false, false],
    }
}

fn dirs_to_dpad(d: [bool; 4]) -> u8 {
    match d {
        [true,  false, false, false] => DPAD_N,
        [true,  true,  false, false] => DPAD_NE,
        [false, true,  false, false] => DPAD_E,
        [false, true,  true,  false] => DPAD_SE,
        [false, false, true,  false] => DPAD_S,
        [false, false, true,  true ] => DPAD_SW,
        [false, false, false, true ] => DPAD_W,
        [true,  false, false, true ] => DPAD_NW,
        _                            => DPAD_NEUTRAL,
    }
}

fn encode_button(btn: &Button, out: &mut u32, dirs: &mut [bool; 4]) {
    match btn {
        Button::Square    => *out |= BTN_SQUARE,
        Button::Cross     => *out |= BTN_CROSS,
        Button::Circle    => *out |= BTN_CIRCLE,
        Button::Triangle  => *out |= BTN_TRIANGLE,
        Button::L1        => *out |= BTN_L1,
        Button::R1        => *out |= BTN_R1,
        Button::L2        => *out |= BTN_L2,
        Button::R2        => *out |= BTN_R2,
        Button::Create    => *out |= BTN_CREATE,
        Button::Options   => *out |= BTN_OPTIONS,
        Button::L3        => *out |= BTN_L3,
        Button::R3        => *out |= BTN_R3,
        Button::PS        => *out |= BTN_PS,
        Button::Touchpad  => *out |= BTN_TOUCHPAD,
        Button::Mute      => *out |= BTN_MUTE,
        Button::DPadUp    => dirs[0] = true,
        Button::DPadRight => dirs[1] = true,
        Button::DPadDown  => dirs[2] = true,
        Button::DPadLeft  => dirs[3] = true,
    }
}


fn remap_buttons(
    buttons:  u32,
    dpad:     u8,
    remap:    &HashMap<Button, Button>,
    disabled: &HashSet<Button>,
) -> (u32, u8) {
    let dirs = dpad_to_dirs(dpad);

    let active: [(Button, bool); 19] = [
        (Button::Square,    buttons & BTN_SQUARE   != 0),
        (Button::Cross,     buttons & BTN_CROSS    != 0),
        (Button::Circle,    buttons & BTN_CIRCLE   != 0),
        (Button::Triangle,  buttons & BTN_TRIANGLE != 0),
        (Button::L1,        buttons & BTN_L1       != 0),
        (Button::R1,        buttons & BTN_R1       != 0),
        (Button::L2,        buttons & BTN_L2       != 0),
        (Button::R2,        buttons & BTN_R2       != 0),
        (Button::Create,    buttons & BTN_CREATE   != 0),
        (Button::Options,   buttons & BTN_OPTIONS  != 0),
        (Button::L3,        buttons & BTN_L3       != 0),
        (Button::R3,        buttons & BTN_R3       != 0),
        (Button::PS,        buttons & BTN_PS       != 0),
        (Button::Touchpad,  buttons & BTN_TOUCHPAD != 0),
        (Button::Mute,      buttons & BTN_MUTE     != 0),
        (Button::DPadUp,    dirs[0]),
        (Button::DPadRight, dirs[1]),
        (Button::DPadDown,  dirs[2]),
        (Button::DPadLeft,  dirs[3]),
    ];

    let mut out_buttons: u32 = 0;
    let mut out_dirs = [false; 4];

    for (btn, pressed) in &active {
        if !pressed              { continue; }
        if disabled.contains(btn) { continue; }
        let target = remap.get(btn).unwrap_or(btn);
        encode_button(target, &mut out_buttons, &mut out_dirs);
    }

    (out_buttons, dirs_to_dpad(out_dirs))
}


