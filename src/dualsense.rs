use std::{thread::sleep, time::{Duration, Instant}};

use anyhow::{anyhow, bail, Context, Result};
use hidapi::{HidApi, HidDevice};
use crc::{Crc, CRC_32_ISO_HDLC};

use crate::common::*;

const OUTPUT_CRC32_SEED: u8 = 0xa2;

const DS_INPUT_REPORT_USB: u8 = 0x01;
const DS_INPUT_REPORT_USB_SIZE: usize = 64;
const DS_INPUT_REPORT_BT: u8 = 0x31;
const DS_INPUT_REPORT_BT_SIZE: usize = 78;

const DS_OUTPUT_REPORT_USB: u8 = 0x02;
const DS_OUTPUT_REPORT_BT: u8 = 0x31;
const DS_OUTPUT_REPORT_BT_SIZE: usize = 78;
const DS_OUTPUT_TAG: u8 = 0x10;

const DS_OUTPUT_VALID_FLAG0_COMPATIBLE_VIBRATION: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG0_HAPTICS_SELECT: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_MOTOR_ENABLE: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_MOTOR_ENABLE: u8 = 1 << 3;
const DS_OUTPUT_VALID_FLAG0_HEADPHONE_VOLUME_ENABLE: u8 = 1 << 4;
const DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE: u8 = 1 << 5;
const DS_OUTPUT_VALID_FLAG0_MICROPHONE_VOLUME_ENABLE: u8 = 1 << 6;
const DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE: u8 = 1 << 7;

const DS_OUTPUT_VALID_FLAG1_MIC_MUTE_LED_CONTROL_ENABLE: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG1_POWER_SAVE_CONTROL_ENABLE: u8 = 1 << 1;
const DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE: u8 = 1 << 2;
const DS_OUTPUT_VALID_FLAG1_RELEASE_LEDS: u8 = 1 << 3;
const DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE: u8 = 1 << 4;
const DS_OUTPUT_VALID_FLAG1_HAPTIC_LOW_PASS_FILTER: u8 = 1 << 5;
const DS_OUTPUT_VALID_FLAG1_VIBRATION_ATTENUATION_ENABLE: u8 = 1 << 6;
const DS_OUTPUT_VALID_FLAG1_AUDIO_CONTROL2_ENABLE: u8 = 1 << 7;

const DS_OUTPUT_VALID_FLAG2_LED_BRIGHTNESS_CONTROL_ENABLE: u8 = 1 << 0;
const DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE: u8 = 1 << 1;

const DS_OUTPUT_POWER_SAVE_CONTROL_AUDIO: u8 = 1 << 3;
const DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE: u8 = 1 << 4;

const DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_ON: u8 = 1 << 0;
const DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OFF: u8 = 1 << 1;

const DS_OUTPUT_AUDIO_OUTPUT_PATH_SHIFT: u8 = 4;

const DS_STATUS_BATTERY_CAPACITY: u8 = 0x0f;
const DS_STATUS_CHARGING: u8 = 0xf0;
const DS_STATUS_CHARGING_SHIFT: u8 = 4;

const DS_FEATURE_REPORT_FW: u8 = 0xf4;
const DS_FEATURE_REPORT_FW_STATUS: u8 = 0xf5;
const DS_BATTERY_THRESHOLD: u8 = 10;

const DS_TRIGGER_EFFECT_OFF: u8 = 0x05;
const DS_TRIGGER_EFFECT_FEEDBACK: u8 = 0x21;

const CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[repr(C, packed)]
struct DualSenseInputReport {
    x: u8,
    y: u8,
    rx: u8,
    ry: u8,
    z: u8,
    rz: u8,
    seq_number: u8,
    buttons: [u8; 4],
    reserved: [u8; 4],
    gyro: [u16; 3],
    accel: [u16; 3],
    sensor_timestamp: u32,
    reserved2: u8
}

#[derive(Clone)]
pub struct BatteryInfo {
    pub capacity: u8,
    pub status: String
}

pub struct DualSense {
    device: HidDevice,
    is_bt: bool,
    output_seq: u8,
    product_id: u16,
    serial: String
}

impl DualSense {
    pub fn new(api: &HidApi, serial: Option<&str>) -> Result<Self> {
        let device_info = api.device_list()
            .find(|info| {
                if info.vendor_id() != DS_VID {
                    return false;
                }

                if info.product_id() != DS_PID && info.product_id() != DSE_PID {
                    return false;
                }

                if let Some(s) = serial {
                    return info.serial_number() == Some(s);
                }

                true
            })
            .ok_or_else(|| {
                if serial.is_some() {
                    anyhow!(
                        "DualSense controller '{}' not found.
                        Check connection and try refreshing",
                        serial.unwrap())
                } else {
                    anyhow!("No DualSense controller found.
                        Please connect your controller via USB or Bluetooth.")
                }
            })?;

        let product_id = device_info.product_id();
        let serial = device_info.serial_number().unwrap_or("Unknown").to_string();
        let device = device_info.open_device(api)?;
        let is_bt = device_info.interface_number() == -1;

        Ok(DualSense { device, is_bt, output_seq: 0, product_id, serial })
    }

    pub fn get_firmware_info(&self) -> Result<(u16, String, String)> {
        let mut buf = vec![0u8; 64];
        buf[0] = 0x20;

        let size = self.device.get_feature_report(&mut buf)
            .context("Failed to read firmware version")?;

        if size < 50 {
            bail!("Feature report too short: {} bytes", size);
        }

        let update_version = u16::from_le_bytes([buf[44], buf[45]]);

        let build_date = String::from_utf8_lossy(&buf[1..12])
            .trim_end_matches('\0')
            .to_string();
        
        let build_time = String::from_utf8_lossy(&buf[12..20])
            .trim_end_matches('\0')
            .to_string();

        Ok((update_version, build_date, build_time))
    }

    pub fn is_bluetooth(&self) -> bool {
        self.is_bt
    }

    pub fn product_id(&self) -> u16 {
        self.product_id
    }

    pub fn serial(&self) -> &str {
        &self.serial
    }

    fn send_output_report(&mut self, data: &mut [u8]) -> Result<()> {
        if self.is_bt {
            let len = data.len();
            let crc = self.calc_crc32(&data[0..len - 4]);
            data[len - 4..len].copy_from_slice(&crc.to_le_bytes());
        }

        self.device.write(data)?;
        Ok(())
    }

    fn calc_crc32(&self, data: &[u8]) -> u32 {
        let mut digest = CRC32.digest();
        digest.update(&[OUTPUT_CRC32_SEED]);
        digest.update(data);
        !digest.finalize()
    }

    fn init_output_report(&mut self) -> Vec<u8> {
        if self.is_bt {
            let mut buf = vec![0u8; DS_OUTPUT_REPORT_BT_SIZE];
            buf[0] = DS_OUTPUT_REPORT_BT;
            buf[1] = self.output_seq << 4;
            buf[2] = DS_OUTPUT_TAG;

            self.output_seq = (self.output_seq + 1) % 16;
            buf
        } else {
            let mut buf = vec![0u8; 63];
            buf[0] = DS_OUTPUT_REPORT_USB;
            buf
        }
    }

    pub fn set_lightbar(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 1] = DS_OUTPUT_VALID_FLAG1_LIGHTBAR_CONTROL_ENABLE;

        let max_brightness = 255u16;

        buf[offset + 44] = ((brightness as u16 * r as u16) / max_brightness) as u8;
        buf[offset + 45] = ((brightness as u16 * g as u16) / max_brightness) as u8;
        buf[offset + 46] = ((brightness as u16 * b as u16) / max_brightness) as u8;

        self.send_output_report(&mut buf)
    }

    pub fn set_lightbar_enabled(&mut self, enabled: bool) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 38] = DS_OUTPUT_VALID_FLAG2_LIGHTBAR_SETUP_CONTROL_ENABLE;
        buf[offset + 41] = if enabled {
            DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_ON
        } else {
            DS_OUTPUT_LIGHTBAR_SETUP_LIGHT_OFF
        };

        self.send_output_report(&mut buf)
    }

    pub fn set_player_leds(&mut self, n: u8) -> Result<()> {
        const PLAYER_LED_PATTERNS: [u8; 8] = [
            0b00000,
            0b00100,
            0b01010,
            0b10101,
            0b11011,
            0b11111,
            0b10001,
            0b01110
        ];

        if n >= PLAYER_LED_PATTERNS.len() as u8 {
            bail!("Invalid player number");
        }

        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 1] = DS_OUTPUT_VALID_FLAG1_PLAYER_INDICATOR_CONTROL_ENABLE;
        buf[offset + 43] = PLAYER_LED_PATTERNS[n as usize];

        self.send_output_report(&mut buf)
    }

    pub fn set_speaker(&mut self, mode: &str) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 0] = DS_OUTPUT_VALID_FLAG0_AUDIO_CONTROL_ENABLE;

        buf[offset + 7] = match mode {
            "internal" => 3 << DS_OUTPUT_AUDIO_OUTPUT_PATH_SHIFT,
            "headphone" => 0,
            "both" => 2 << DS_OUTPUT_AUDIO_OUTPUT_PATH_SHIFT,
            _ => 0
        };

        self.send_output_report(&mut buf)
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<()> { 
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        let max_volume = 255u16;

        buf[offset] = DS_OUTPUT_VALID_FLAG0_HEADPHONE_VOLUME_ENABLE;
        buf[offset + 4] = (volume as u16 * 0x7f / max_volume) as u8;

        buf[offset] |= DS_OUTPUT_VALID_FLAG0_SPEAKER_VOLUME_ENABLE;
        buf[offset + 5] = (volume as u16 * 0x64 / max_volume) as u8;
        
        self.send_output_report(&mut buf)
    }

    pub fn set_vibration(&mut self, rumble: u8, trigger: u8) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 1]  = DS_OUTPUT_VALID_FLAG1_VIBRATION_ATTENUATION_ENABLE;
        buf[offset + 36] = (rumble & 0x07) | ((trigger & 0x07) << 4);

        self.send_output_report(&mut buf)
    }

    pub fn set_mic(&mut self, enabled: bool) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };
        
        buf[offset + 1] = DS_OUTPUT_VALID_FLAG1_POWER_SAVE_CONTROL_ENABLE;
        if enabled {
            buf[offset + 9] &= !DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE;
            buf[offset + 9] &= !DS_OUTPUT_POWER_SAVE_CONTROL_AUDIO;
        } else {
            buf[offset + 9] |= DS_OUTPUT_POWER_SAVE_CONTROL_MIC_MUTE;
        }

        self.send_output_report(&mut buf)
    }

    pub fn set_mic_led(&mut self, state: MicLedState) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };

        buf[offset + 1] = DS_OUTPUT_VALID_FLAG1_MIC_MUTE_LED_CONTROL_ENABLE;
        buf[offset + 8] = match state {
            MicLedState::Off => 0,
            MicLedState::On => 1,
            MicLedState::Pulse => 2
        };

        self.send_output_report(&mut buf)
    }

    pub fn set_trigger_effect(
        &mut self,
        left: bool, right: bool,
        mode: u8, params: &[u8]
    ) -> Result<()> {
        let mut buf = self.init_output_report();
        let offset = if self.is_bt { 3 } else { 1 };
        
        if right {
            buf[offset] |= DS_OUTPUT_VALID_FLAG0_RIGHT_TRIGGER_MOTOR_ENABLE;
        }

        if left {
            buf[offset] |= DS_OUTPUT_VALID_FLAG0_LEFT_TRIGGER_MOTOR_ENABLE;
        }

        buf[offset + 10] = mode;
        for (i, &p) in params.iter().enumerate().take(10) {
            buf[offset + 11 + i] = p;
        }

        buf[offset + 21] = mode;
        for (i, &p) in params.iter().enumerate().take(10) {
            buf[offset + 22 + i] = p;
        }

        self.send_output_report(&mut buf)
    }

    pub fn set_trigger_off(&mut self) -> Result<()> {
        self.set_trigger_effect(true, true, DS_TRIGGER_EFFECT_OFF, &[0; 10])
    }

    pub fn get_battery(&mut self) -> Result<BatteryInfo> {
        let mut buf = vec![0u8; DS_INPUT_REPORT_BT_SIZE];
        let size = self.device.read_timeout(&mut buf, 1000)?;

        if size == 0 {
            bail!("Timeout");
        }

        let (report, report_size, status_offset) = if self.is_bt {
            (DS_INPUT_REPORT_BT, DS_INPUT_REPORT_BT_SIZE, 54)
        } else {
            (DS_INPUT_REPORT_USB, DS_INPUT_REPORT_USB_SIZE, 53)
        }; 

        if buf[0] != report || size != report_size {
            bail!("Invalid report received");
        }

        let status_byte = buf[status_offset];

        let bat_data = status_byte & DS_STATUS_BATTERY_CAPACITY;
        let charging_status = (status_byte & DS_STATUS_CHARGING) >> DS_STATUS_CHARGING_SHIFT;

        let (capacity, status) = match charging_status {
            0x0 => ((bat_data * 10 + 5).min(100), "Discharging"),
            0x1 => ((bat_data * 10 + 5).min(100), "Charging"),
            0x2 => (100, "Full"),
            0xa | 0xb => (0, "Not charging"),
            _ => (0, "Unknown")
        };

        Ok(BatteryInfo { capacity, status: status.to_string() })
    }

    pub fn update_firmware(
        &mut self,
        firmware_data: &[u8],
        progress_callback: impl Fn(u32) + Send + 'static
    ) -> Result<()> {
        if self.is_bt {
            bail!("Firmware update not supported over Bluetooth.");
        }

        if firmware_data.len() != FIRMWARE_SIZE {
            bail!("Invalid firmware size: {} bytes (expected {})",
                    firmware_data.len(), FIRMWARE_SIZE);
        }

        let battery = self.get_battery()?;
        if battery.capacity < DS_BATTERY_THRESHOLD {
            bail!("Battery too low: {}% (need at least {}%)", 
                battery.capacity, DS_BATTERY_THRESHOLD);
        }

        self.check_firmware_compatibility(firmware_data)?;

        progress_callback(0);
        
        self.firmware_start(firmware_data)?;
        
        progress_callback(5);
        
        self.firmware_write(firmware_data, &progress_callback)?;

        progress_callback(95);
        
        self.firmware_verify()?;

        progress_callback(98);

        self.firmware_finale()?;
        
        progress_callback(100);
        Ok(())
    }

    fn check_firmware_compatibility(&self, firmware_data: &[u8]) -> Result<()> {
        if firmware_data.len() < 0x80 {
            bail!("Firmware file too small");
        }

        let fw_product_id = u16::from_le_bytes([firmware_data[0x62], firmware_data[0x63]]);
        let fw_version = u16::from_le_bytes([firmware_data[0x78], firmware_data[0x79]]);

        if fw_product_id != self.product_id {
            bail!(
                "Firmware incompatible. Firmware device: 0x{:04X}, Connected device: 0x{:04X}",
                fw_product_id, self.product_id
            );
        }

        let mut buf = vec![0u8; 64];
        buf[0] = 0x20;
        
        match self.device.get_feature_report(&mut buf) {
            Ok(size) if size == 64 => {
                let current_version = u16::from_le_bytes([buf[44], buf[45]]);
                println!("Updating firmware for {} from 0x{:04X} to 0x{:04X}",
                    if self.product_id == DS_PID { "DualSense" } else { "DualSense Edge" },
                    current_version, fw_version);
            }
            _ => {
                eprintln!("Warning: Could not read current firmware version");
            }
        }

        Ok(())
    }

    fn send_firmware_feature(&self, buf: &[u8]) -> Result<()> {
        self.device.send_feature_report(buf)
            .map_err(|e| anyhow!("Failed to send firmware data: {}.
                    Controller may have disconnected.", e))
    }

    fn firmware_wait_status(&self, expected: u8) -> Result<()> {
        let start = Instant::now();
        loop {
            if start.elapsed() > Duration::from_secs(30) {
                bail!("Firmware update timeout");
            }
            sleep(Duration::from_millis(10));
            let mut buf = vec![0u8; 64];
            buf[0] = DS_FEATURE_REPORT_FW_STATUS;
            
            self.device.get_feature_report(&mut buf)?;

            let phase = buf[1];
            let status = buf[2];

            if phase != expected {
                bail!("Unexpected phase: 0x{:02x} (expected 0x{:02x})", phase, expected);
            }

            match status {
                0x00 => return Ok(()),
                0x01 | 0x10 => continue,
                0x02 => bail!("Err 0x{:02x}: Invalid firmware size", status),
                0x03 => {
                    if expected == 0x01 {
                        return Ok(());
                    }
                    bail!("Err 0x{:02x}: Invalid firmware", status);
                },
                0x04 => {
                    if expected == 0x0 || expected == 0x02 {
                        sleep(Duration::from_secs(10));
                        continue;
                    }
                    bail!("Err 0x{:02x}: Invalid firmware", status);
                },
                0x11 => bail!("Err 0x{:02x}: Invalid firmware", status),
                0xFF => bail!("Err 0x{:02x}: Internal error", status),
                _ => bail!("Unknown error: 0x{:02x}", status)
            }
        }
    }

    fn firmware_start(&mut self, firmware_data: &[u8]) -> Result<()> {
        for offset in (0..256).step_by(57) {
            let remaining = 256 - offset;
            let chunk_size = remaining.min(57);

            let mut buf = vec![0u8; 64];
            buf[0] = DS_FEATURE_REPORT_FW;
            buf[2] = chunk_size as u8;
            buf[3..3+chunk_size].copy_from_slice(&firmware_data[offset..offset+chunk_size]);

            self.send_firmware_feature(&buf)?;

            if offset == 0 {
                sleep(Duration::from_millis(50));
            }
        }

        self.firmware_wait_status(0x00)
    }

    fn firmware_write(
        &mut self,
        firmware_data: &[u8],
        progress_callback: impl Fn(u32)
    ) -> Result<()> {
        let total_size = firmware_data.len();

        for offset in (0..total_size).step_by(0x8000) {
            for chunk_offset in (0..0x8000).step_by(57) {
                let remaining = 0x8000 - chunk_offset;
                let packet_size = remaining.min(57);
                let global_offset = offset + chunk_offset;

                if global_offset >= total_size {
                    break;
                }

                let actual_size = (total_size - global_offset).min(packet_size);

                let mut buf = vec![0u8; 64];
                buf[0] = DS_FEATURE_REPORT_FW;
                buf[1] = 0x01;
                buf[2] = actual_size as u8;
                buf[3..3+actual_size].copy_from_slice(
                    &firmware_data[global_offset..global_offset+actual_size]);

                self.send_firmware_feature(&buf)?;
                self.firmware_wait_status(0x01)?;
                sleep(Duration::from_millis(10));

                let progress = (global_offset.saturating_sub(256) * 90)
                    / (total_size - 256).max(1) + 5;

                progress_callback(progress.min(95) as u32);
            }
        }

        Ok(())
    }

    fn firmware_verify(&mut self) -> Result<()> {
        let mut buf = vec![0u8; 64];
        buf[0] = DS_FEATURE_REPORT_FW;
        buf[1] = 0x02;

        self.send_firmware_feature(&buf)?;
        self.firmware_wait_status(0x02)
    }

    fn firmware_finale(&mut self) -> Result<()> {
        let mut buf = vec![0u8; 64];
        buf[0] = DS_FEATURE_REPORT_FW;
        buf[1] = 0x03;

        self.send_firmware_feature(&buf)
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum MicLedState {
    Off,
    On,
    Pulse
}

pub fn list_devices(api: &HidApi) -> Vec<String> {
    api.device_list()
        .filter(|info| {
            info.vendor_id() == DS_VID && 
            (info.product_id() == DS_PID || info.product_id() == DSE_PID)
        })
        .map(|info| {
            let connection = if info.interface_number() == -1 { "Bluetooth" } else { "USB" };
            let serial = info.serial_number().unwrap_or("Unknown");
            format!("{} ({})", serial, connection)
        })
        .collect()
}

