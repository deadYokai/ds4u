use std::collections::HashMap;

use anyhow::{Context, Result};
use evdev::{
    AbsInfo, AbsoluteAxisCode, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode,
    UinputAbsSetup, uinput::VirtualDevice,
};

use crate::inputs::{
    BTN_CIRCLE, BTN_CREATE, BTN_CROSS, BTN_L1, BTN_L2, BTN_L3, BTN_OPTIONS, BTN_PS, BTN_R1, BTN_R2,
    BTN_R3, BTN_SQUARE, BTN_TRIANGLE, ControllerState, DPAD_E, DPAD_N, DPAD_NE, DPAD_NW, DPAD_S,
    DPAD_SE, DPAD_SW, DPAD_W,
};

const DS_EVDEV_VERSION: u16 = 0x8111;

const KEYS: [KeyCode; 13] = [
    KeyCode::BTN_SOUTH,
    KeyCode::BTN_EAST,
    KeyCode::BTN_NORTH,
    KeyCode::BTN_WEST,
    KeyCode::BTN_TL,
    KeyCode::BTN_TR,
    KeyCode::BTN_TL2,
    KeyCode::BTN_TR2,
    KeyCode::BTN_SELECT,
    KeyCode::BTN_START,
    KeyCode::BTN_MODE,
    KeyCode::BTN_THUMBL,
    KeyCode::BTN_THUMBR,
];

// (axis, min, max, flat)
const AXES: [(AbsoluteAxisCode, i32, i32, i32); 8] = [
    (AbsoluteAxisCode::ABS_X, -32768, 32767, 1000),
    (AbsoluteAxisCode::ABS_Y, -32768, 32767, 1000),
    (AbsoluteAxisCode::ABS_RX, -32768, 32767, 1000),
    (AbsoluteAxisCode::ABS_RY, -32768, 32767, 1000),
    (AbsoluteAxisCode::ABS_Z, 0, 255, 0),
    (AbsoluteAxisCode::ABS_RZ, 0, 255, 0),
    (AbsoluteAxisCode::ABS_HAT0X, -1, 1, 0),
    (AbsoluteAxisCode::ABS_HAT0Y, -1, 1, 0),
];

pub struct VirtualGamepad {
    dev: VirtualDevice,
    last: HashMap<(u16, u16), i32>,
    out: Vec<InputEvent>,
}

impl VirtualGamepad {
    pub fn new(vendor: u16, product: u16, name: &str) -> Result<Self> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for k in KEYS {
            keys.insert(k);
        }

        let mut builder = VirtualDevice::builder()
            .context(
                "opening /dev/uinput (is the uinput kernel module loaded, \
                 and do you have write permission on it?)",
            )?
            .name(name)
            .input_id(InputId::new(
                BusType::BUS_USB,
                vendor,
                product,
                DS_EVDEV_VERSION,
            ))
            .with_keys(&keys)
            .context("registering buttons")?;

        for (axis, min, max, flat) in AXES {
            let setup = UinputAbsSetup::new(axis, AbsInfo::new(0, min, max, 0, flat, 0));
            builder = builder
                .with_absolute_axis(&setup)
                .context("registering axes")?;
        }

        let dev = builder.build().context("creating uinput device")?;

        Ok(Self {
            dev,
            last: HashMap::new(),
            out: Vec::with_capacity(24),
        })
    }

    fn emit_abs(&mut self, axis: AbsoluteAxisCode, value: i32) {
        self.emit(EventType::ABSOLUTE, axis.0, value);
    }

    fn emit_key(&mut self, key: KeyCode, pressed: bool) {
        self.emit(EventType::KEY, key.0, pressed as i32);
    }

    fn emit(&mut self, kind: EventType, code: u16, value: i32) {
        if self.last.insert((kind.0, code), value) != Some(value) {
            self.out.push(InputEvent::new(kind.0, code, value));
        }
    }

    fn axis(raw: u8) -> i32 {
        (((raw as i32) - 128) * 256).clamp(-32768, 32767)
    }

    pub fn update(&mut self, s: &ControllerState) -> Result<()> {
        self.out.clear();

        self.emit_abs(AbsoluteAxisCode::ABS_X, Self::axis(s.left_x));
        self.emit_abs(AbsoluteAxisCode::ABS_Y, Self::axis(s.left_y));
        self.emit_abs(AbsoluteAxisCode::ABS_RX, Self::axis(s.right_x));
        self.emit_abs(AbsoluteAxisCode::ABS_RY, Self::axis(s.right_y));
        self.emit_abs(AbsoluteAxisCode::ABS_Z, s.l2 as i32);
        self.emit_abs(AbsoluteAxisCode::ABS_RZ, s.r2 as i32);

        let (hx, hy) = match s.dpad {
            DPAD_N => (0, -1),
            DPAD_NE => (1, -1),
            DPAD_E => (1, 0),
            DPAD_SE => (1, 1),
            DPAD_S => (0, 1),
            DPAD_SW => (-1, 1),
            DPAD_W => (-1, 0),
            DPAD_NW => (-1, -1),
            _ => (0, 0),
        };
        self.emit_abs(AbsoluteAxisCode::ABS_HAT0X, hx);
        self.emit_abs(AbsoluteAxisCode::ABS_HAT0Y, hy);

        let b = s.buttons;
        self.emit_key(KeyCode::BTN_SOUTH, b & BTN_CROSS != 0);
        self.emit_key(KeyCode::BTN_EAST, b & BTN_CIRCLE != 0);
        self.emit_key(KeyCode::BTN_WEST, b & BTN_SQUARE != 0);
        self.emit_key(KeyCode::BTN_NORTH, b & BTN_TRIANGLE != 0);
        self.emit_key(KeyCode::BTN_TL, b & BTN_L1 != 0);
        self.emit_key(KeyCode::BTN_TR, b & BTN_R1 != 0);
        self.emit_key(KeyCode::BTN_TL2, b & BTN_L2 != 0);
        self.emit_key(KeyCode::BTN_TR2, b & BTN_R2 != 0);
        self.emit_key(KeyCode::BTN_SELECT, b & BTN_CREATE != 0);
        self.emit_key(KeyCode::BTN_START, b & BTN_OPTIONS != 0);
        self.emit_key(KeyCode::BTN_MODE, b & BTN_PS != 0);
        self.emit_key(KeyCode::BTN_THUMBL, b & BTN_L3 != 0);
        self.emit_key(KeyCode::BTN_THUMBR, b & BTN_R3 != 0);

        if self.out.is_empty() {
            return Ok(());
        }
        self.dev.emit(&self.out)?;
        Ok(())
    }
}
