#![allow(dead_code)]

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::mem::{size_of, zeroed};
use std::os::unix::io::AsRawFd;

use anyhow::{Context, Result, bail};

use crate::inputs::{
    BTN_CIRCLE, BTN_CREATE, BTN_CROSS, BTN_L1, BTN_L2, BTN_L3, BTN_OPTIONS, BTN_PS, BTN_R1, BTN_R2,
    BTN_R3, BTN_SQUARE, BTN_TRIANGLE, ControllerState, DPAD_E, DPAD_N, DPAD_NE, DPAD_NW, DPAD_S,
    DPAD_SE, DPAD_SW, DPAD_W,
};

const EV_SYN: u16 = 0x00;
const EV_KEY: u16 = 0x01;
const EV_ABS: u16 = 0x03;

const BTN_SOUTH: u16 = 0x130;
const BTN_EAST: u16 = 0x131;
const BTN_NORTH: u16 = 0x133;
const BTN_WEST: u16 = 0x134;
const BTN_TL: u16 = 0x136;
const BTN_TR: u16 = 0x137;
const BTN_TL2: u16 = 0x138;
const BTN_TR2: u16 = 0x139;
const BTN_SELECT: u16 = 0x13a;
const BTN_START: u16 = 0x13b;
const BTN_MODE: u16 = 0x13c;
const BTN_THUMBL: u16 = 0x13d;
const BTN_THUMBR: u16 = 0x13e;

const ABS_X: u16 = 0x00;
const ABS_Y: u16 = 0x01;
const ABS_Z: u16 = 0x02;
const ABS_RX: u16 = 0x03;
const ABS_RY: u16 = 0x04;
const ABS_RZ: u16 = 0x05;
const ABS_HAT0X: u16 = 0x10;
const ABS_HAT0Y: u16 = 0x11;

const BUS_USB: u16 = 0x03;

const DS_EVDEV_VERSION: u16 = 0x8111;

const UINPUT_IOCTL_BASE: u64 = b'U' as u64;
const UINPUT_MAX_NAME_SIZE: usize = 80;
const ABS_CNT: usize = 64;

fn ioc(dir: u64, nr: u64, size: u64) -> u64 {
    (dir << 30) | (UINPUT_IOCTL_BASE << 8) | nr | (size << 16)
}
const IOC_NONE: u64 = 0;
const IOC_WRITE: u64 = 1;

fn ui_set_evbit() -> u64 {
    ioc(IOC_WRITE, 100, size_of::<i32>() as u64)
}
fn ui_set_keybit() -> u64 {
    ioc(IOC_WRITE, 101, size_of::<i32>() as u64)
}
fn ui_set_absbit() -> u64 {
    ioc(IOC_WRITE, 103, size_of::<i32>() as u64)
}
fn ui_dev_create() -> u64 {
    ioc(IOC_NONE, 1, 0)
}
fn ui_dev_destroy() -> u64 {
    ioc(IOC_NONE, 2, 0)
}

#[repr(C)]
struct InputId {
    bustype: u16,
    vendor: u16,
    product: u16,
    version: u16,
}

#[repr(C)]
struct UinputUserDev {
    name: [u8; UINPUT_MAX_NAME_SIZE],
    id: InputId,
    ff_effects_max: u32,
    absmax: [i32; ABS_CNT],
    absmin: [i32; ABS_CNT],
    absfuzz: [i32; ABS_CNT],
    absflat: [i32; ABS_CNT],
}

#[repr(C)]
struct InputEvent {
    tv_sec: i64,
    tv_usec: i64,
    kind: u16,
    code: u16,
    value: i32,
}

pub struct VirtualGamepad {
    file: File,
    last: HashMap<(u16, u16), i32>,
    out: Vec<u8>,
}

impl VirtualGamepad {
    pub fn new(vendor: u16, product: u16) -> Result<Self> {
        let file = OpenOptions::new().write(true).open("/dev/uinput").context(
            "opening /dev/uinput (is the uinput kernel module loaded, \
                 and do you have write permission on it?)",
        )?;
        let fd = file.as_raw_fd();

        unsafe {
            for bit in [EV_KEY, EV_ABS] {
                if libc::ioctl(fd, ui_set_evbit() as _, bit as i32) < 0 {
                    bail!("UI_SET_EVBIT failed");
                }
            }
            for key in [
                BTN_SOUTH, BTN_EAST, BTN_NORTH, BTN_WEST, BTN_TL, BTN_TR, BTN_TL2, BTN_TR2,
                BTN_SELECT, BTN_START, BTN_MODE, BTN_THUMBL, BTN_THUMBR,
            ] {
                if libc::ioctl(fd, ui_set_keybit() as _, key as i32) < 0 {
                    bail!("UI_SET_KEYBIT failed");
                }
            }
            for a in [
                ABS_X, ABS_Y, ABS_Z, ABS_RX, ABS_RY, ABS_RZ, ABS_HAT0X, ABS_HAT0Y,
            ] {
                if libc::ioctl(fd, ui_set_absbit() as _, a as i32) < 0 {
                    bail!("UI_SET_ABSBIT failed");
                }
            }
        }

        let mut dev: UinputUserDev = unsafe { zeroed() };
        let name = b"DS4U DualSense Wireless Controller\0";
        dev.name[..name.len()].copy_from_slice(name);
        dev.id = InputId {
            bustype: BUS_USB,
            vendor,
            product,
            version: DS_EVDEV_VERSION,
        };

        let mut set_range = |axis: u16, min: i32, max: i32, flat: i32| {
            dev.absmin[axis as usize] = min;
            dev.absmax[axis as usize] = max;
            dev.absflat[axis as usize] = flat;
        };
        set_range(ABS_X, -32768, 32767, 1000);
        set_range(ABS_Y, -32768, 32767, 1000);
        set_range(ABS_RX, -32768, 32767, 1000);
        set_range(ABS_RY, -32768, 32767, 1000);
        set_range(ABS_Z, 0, 255, 0);
        set_range(ABS_RZ, 0, 255, 0);
        set_range(ABS_HAT0X, -1, 1, 0);
        set_range(ABS_HAT0Y, -1, 1, 0);

        let bytes = unsafe {
            std::slice::from_raw_parts(&dev as *const _ as *const u8, size_of::<UinputUserDev>())
        };
        (&file)
            .write_all(bytes)
            .context("writing uinput_user_dev")?;

        if unsafe { libc::ioctl(fd, ui_dev_create() as _) } < 0 {
            bail!("UI_DEV_CREATE failed");
        }

        Ok(Self {
            file,
            last: HashMap::new(),
            out: Vec::with_capacity(24 * 20),
        })
    }

    fn push_raw(&mut self, kind: u16, code: u16, value: i32) {
        let ev = InputEvent {
            tv_sec: 0,
            tv_usec: 0,
            kind,
            code,
            value,
        };
        let bytes = unsafe {
            std::slice::from_raw_parts(&ev as *const _ as *const u8, size_of::<InputEvent>())
        };
        self.out.extend_from_slice(bytes);
    }

    fn emit(&mut self, kind: u16, code: u16, value: i32) {
        if self.last.insert((kind, code), value) != Some(value) {
            self.push_raw(kind, code, value);
        }
    }

    fn axis(raw: u8) -> i32 {
        (((raw as i32) - 128) * 256).clamp(-32768, 32767)
    }

    pub fn update(&mut self, s: &ControllerState) -> Result<()> {
        self.out.clear();
        self.emit(EV_ABS, ABS_X, Self::axis(s.left_x));
        self.emit(EV_ABS, ABS_Y, Self::axis(s.left_y));
        self.emit(EV_ABS, ABS_RX, Self::axis(s.right_x));
        self.emit(EV_ABS, ABS_RY, Self::axis(s.right_y));
        self.emit(EV_ABS, ABS_Z, s.l2 as i32);
        self.emit(EV_ABS, ABS_RZ, s.r2 as i32);

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
        self.emit(EV_ABS, ABS_HAT0X, hx);
        self.emit(EV_ABS, ABS_HAT0Y, hy);

        let b = s.buttons;
        self.emit(EV_KEY, BTN_SOUTH, (b & BTN_CROSS != 0) as i32);
        self.emit(EV_KEY, BTN_EAST, (b & BTN_CIRCLE != 0) as i32);
        self.emit(EV_KEY, BTN_WEST, (b & BTN_SQUARE != 0) as i32);
        self.emit(EV_KEY, BTN_NORTH, (b & BTN_TRIANGLE != 0) as i32);
        self.emit(EV_KEY, BTN_TL, (b & BTN_L1 != 0) as i32);
        self.emit(EV_KEY, BTN_TR, (b & BTN_R1 != 0) as i32);
        self.emit(EV_KEY, BTN_TL2, (b & BTN_L2 != 0) as i32);
        self.emit(EV_KEY, BTN_TR2, (b & BTN_R2 != 0) as i32);
        self.emit(EV_KEY, BTN_SELECT, (b & BTN_CREATE != 0) as i32);
        self.emit(EV_KEY, BTN_START, (b & BTN_OPTIONS != 0) as i32);
        self.emit(EV_KEY, BTN_MODE, (b & BTN_PS != 0) as i32);
        self.emit(EV_KEY, BTN_THUMBL, (b & BTN_L3 != 0) as i32);
        self.emit(EV_KEY, BTN_THUMBR, (b & BTN_R3 != 0) as i32);

        if self.out.is_empty() {
            return Ok(());
        }
        self.push_raw(EV_SYN, 0, 0);
        (&self.file).write_all(&self.out)?;
        Ok(())
    }
}

impl Drop for VirtualGamepad {
    fn drop(&mut self) {
        let fd = self.file.as_raw_fd();
        unsafe {
            libc::ioctl(fd, ui_dev_destroy() as _);
        }
    }
}
