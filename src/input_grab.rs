use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use evdev::Device;

const TAG: &str = "[ds4u daemon]";

pub struct InputGrab {
    devices: Vec<Device>,
}

impl InputGrab {
    pub fn acquire_for(hidraw_path: &str) -> Self {
        let mut devices = Vec::new();

        let Some(target) = hid_parent_syspath(hidraw_path) else {
            eprintln!("{TAG} input grab: no HID parent found for {hidraw_path}");
            return Self { devices };
        };

        let mut en = match udev::Enumerator::new() {
            Ok(e) => e,
            Err(e) => {
                eprintln!("{TAG} input grab: udev enumerator failed: {e}");
                return Self { devices };
            }
        };
        if en.match_subsystem("input").is_err() {
            return Self { devices };
        }
        let found = match en.scan_devices() {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{TAG} input grab: scan failed: {e}");
                return Self { devices };
            }
        };

        for dev in found {
            let Some(devnode) = dev.devnode() else {
                continue;
            };
            let is_event = devnode
                .file_name()
                .and_then(OsStr::to_str)
                .is_some_and(|n| n.starts_with("event"));
            if !is_event {
                continue;
            }

            if dev.syspath().to_string_lossy().contains("/virtual/") {
                continue;
            }

            let Ok(Some(parent)) = dev.parent_with_subsystem("hid") else {
                continue;
            };
            if parent.syspath() != target {
                continue;
            }

            let mut device = match Device::open(devnode) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("{TAG} could not open {}: {}", devnode.display(), e);
                    continue;
                }
            };

            match device.grab() {
                Ok(()) => {
                    println!("{TAG} grabbed {} (native input hidden)", devnode.display());
                    devices.push(device);
                }
                Err(e) => eprintln!(
                    "{TAG} could not grab {} ({e}); already grabbed by something else?",
                    devnode.display()
                ),
            }
        }

        Self { devices }
    }
}

fn hid_parent_syspath(hidraw_path: &str) -> Option<PathBuf> {
    let mut en = udev::Enumerator::new().ok()?;
    en.match_subsystem("hidraw").ok()?;
    for dev in en.scan_devices().ok()? {
        if dev.devnode().is_some_and(|n| n == Path::new(hidraw_path)) {
            let parent = dev.parent_with_subsystem("hid").ok()??;
            return Some(parent.syspath().to_path_buf());
        }
    }
    None
}

impl Drop for InputGrab {
    fn drop(&mut self) {
        for d in &mut self.devices {
            let _ = d.ungrab();
        }
    }
}
