use std::{
    env,
    io::{self, BufRead, BufReader, Write},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::{
    common::{HapticPattern, LightbarEffect, MicLedState},
    dualsense::BatteryInfo,
    inputs::ControllerState,
    profiles::Profile,
    transform::{self, InputTransform},
};

#[cfg(unix)]
mod transport {
    use std::{
        io,
        os::unix::net::{UnixListener, UnixStream},
        path::PathBuf,
        time::Duration,
    };
    pub type Addr = PathBuf;
    pub type Stream = UnixStream;
    pub type Listener = UnixListener;
    pub fn connect(addr: &Addr) -> io::Result<Stream> {
        UnixStream::connect(addr)
    }
    pub fn bind(addr: &Addr) -> io::Result<Listener> {
        UnixListener::bind(addr)
    }
    pub fn set_timeout(s: &Stream, d: Duration) -> io::Result<()> {
        s.set_read_timeout(Some(d))
    }
    pub fn addr_to_string(addr: &Addr) -> String {
        addr.display().to_string()
    }
}

#[cfg(not(unix))]
mod transport {
    use std::{
        io,
        net::{SocketAddr, TcpListener, TcpStream},
        time::Duration,
    };
    pub type Addr = SocketAddr;
    pub type Stream = TcpStream;
    pub type Listener = TcpListener;
    pub fn connect(addr: &Addr) -> io::Result<Stream> {
        TcpStream::connect(addr)
    }
    pub fn bind(addr: &Addr) -> io::Result<Listener> {
        TcpListener::bind(addr)
    }
    pub fn set_timeout(s: &Stream, d: Duration) -> io::Result<()> {
        s.set_read_timeout(Some(d))
    }
    pub fn addr_to_string(addr: &Addr) -> String {
        addr.to_string()
    }
}

pub type DaemonAddr = transport::Addr;
pub type DaemonStream = transport::Stream;
pub type DaemonListener = transport::Listener;

#[cfg(unix)]
pub fn daemon_endpoint() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(env::temp_dir)
        .join("ds4u.socket")
}

#[cfg(not(unix))]
pub fn daemon_endpoint() -> DaemonAddr {
    "127.0.0.1:45623".parse().expect("hardcoded addr is valid")
}

#[inline]
pub fn socket_path() -> DaemonAddr {
    daemon_endpoint()
}

pub fn bind_daemon(addr: &DaemonAddr) -> io::Result<DaemonListener> {
    transport::bind(addr)
}

#[cfg(unix)]
pub fn cleanup_endpoint(addr: &DaemonAddr) {
    if addr.exists() {
        use std::fs;
        let _ = fs::remove_file(addr);
    }
}

#[cfg(not(unix))]
pub fn cleanup_endpoint(_addr: &DaemonAddr) {}

pub fn addr_display(addr: &DaemonAddr) -> String {
    transport::addr_to_string(addr)
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub enum DaemonCommand {
    Ping,
    GetBattery,
    GetInputState,
    GetFirmwareInfo,
    GetControllerInfo,
    SetLightbar {
        r: u8,
        g: u8,
        b: u8,
        brightness: u8,
    },
    SetLightbarEnabled {
        enabled: bool,
    },
    SetPlayerLeds {
        leds: u8,
    },
    SetMic {
        enabled: bool,
    },
    SetMicLed {
        state: MicLedState,
    },
    SetTriggerOff,
    SetTriggerEffect {
        right: bool,
        left: bool,
        effect_type: u8,
        params: [u8; 10],
    },
    SetTriggerEffects {
        left: Option<(u8, [u8; 10])>,
        right: Option<(u8, [u8; 10])>,
    },
    SetVibration {
        rumble: u8,
        trigger: u8,
    },
    SetSpeaker {
        mode: String,
    },
    SetVolume {
        volume: u8,
    },
    SetUpdateMode {
        active: bool,
    },
    SetInputTransform {
        transform: InputTransform,
    },
    ClearInputTransform,
    SetLightbarEffect {
        effect: LightbarEffect,
    },
    SetHapticPattern {
        pattern: HapticPattern,
        strength: u8,
        speed: f32,
    },
    SetGyro {
        enabled: bool,
        smoothing: f32,
        sensitivity: f32,
    },
    SwitchProfile {
        name: String,
    },
    ListProfiles,
    ReloadProfile,
    SaveProfile {
        profile: Profile,
    },
    DeleteProfile {
        name: String,
    },
    GetActiveProfile,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DaemonResponse {
    Pong,
    Ok,
    Error {
        message: String,
    },
    Battery(BatteryInfo),
    InputState(ControllerState),
    FirmwareInfo {
        version: u16,
        build_date: String,
        build_time: String,
    },
    ControllerInfo {
        serial: String,
        product_id: u16,
        is_bt: bool,
    },
    NoDevice,
    ProfileList {
        profiles: Vec<String>,
    },
    ActiveProfile {
        name: String,
    },
}

pub struct IpcClient {
    pub addr: DaemonAddr,
    reader: BufReader<DaemonStream>,
    writer: DaemonStream,
}

impl IpcClient {
    pub fn connect(addr: &DaemonAddr) -> Result<Self> {
        let stream = transport::connect(addr)?;
        transport::set_timeout(&stream, Duration::from_secs(5))?;
        let writer = stream.try_clone()?;

        Ok(Self {
            addr: addr.clone(),
            reader: BufReader::new(stream),
            writer,
        })
    }

    pub fn try_connect(addr: &DaemonAddr) -> Option<Self> {
        let mut c = Self::connect(addr).ok()?;
        c.send(DaemonCommand::Ping).ok()?;
        matches!(c.recv().ok()?, DaemonResponse::Pong).then_some(c)
    }

    pub fn send(&mut self, cmd: DaemonCommand) -> Result<()> {
        let mut line = serde_json::to_string(&cmd)?;
        line.push('\n');
        self.writer.write_all(line.as_bytes())?;
        Ok(())
    }

    pub fn recv(&mut self) -> Result<DaemonResponse> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        if line.is_empty() {
            bail!("Daemon closed the connection");
        }
        Ok(serde_json::from_str(line.trim())?)
    }

    pub fn request(&mut self, cmd: DaemonCommand) -> Result<DaemonResponse> {
        self.send(cmd)?;
        self.recv()
    }

    pub fn get_battery(&mut self) -> Result<BatteryInfo> {
        match self.request(DaemonCommand::GetBattery)? {
            DaemonResponse::Battery(b) => Ok(b),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }

    pub fn get_input_state(&mut self) -> Result<ControllerState> {
        match self.request(DaemonCommand::GetInputState)? {
            DaemonResponse::InputState(s) => Ok(s),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }

    pub fn get_firmware_info(&mut self) -> Result<(u16, String, String)> {
        match self.request(DaemonCommand::GetFirmwareInfo)? {
            DaemonResponse::FirmwareInfo {
                version,
                build_date,
                build_time,
            } => Ok((version, build_date, build_time)),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }

    pub fn get_controller_info(&mut self) -> Result<Option<(String, u16, bool)>> {
        match self.request(DaemonCommand::GetControllerInfo)? {
            DaemonResponse::ControllerInfo {
                serial,
                product_id,
                is_bt,
            } => Ok(Some((serial, product_id, is_bt))),
            DaemonResponse::NoDevice => Ok(None),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }

    pub fn set_lightbar(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        self.request(DaemonCommand::SetLightbar {
            r,
            g,
            b,
            brightness,
        })
        .map(|_| ())
    }

    pub fn set_lightbar_enabled(&mut self, enabled: bool) -> Result<()> {
        match self.request(DaemonCommand::SetLightbarEnabled { enabled })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_player_leds(&mut self, leds: u8) -> Result<()> {
        match self.request(DaemonCommand::SetPlayerLeds { leds })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_mic(&mut self, enabled: bool) -> Result<()> {
        match self.request(DaemonCommand::SetMic { enabled })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_mic_led(&mut self, state: MicLedState) -> Result<()> {
        match self.request(DaemonCommand::SetMicLed { state })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_trigger_off(&mut self) -> Result<()> {
        match self.request(DaemonCommand::SetTriggerOff)? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_trigger_effect(
        &mut self,
        right: bool,
        left: bool,
        effect_type: u8,
        params: [u8; 10],
    ) -> Result<()> {
        match self.request(DaemonCommand::SetTriggerEffect {
            right,
            left,
            effect_type,
            params,
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_trigger_effects(
        &mut self,
        left: Option<(u8, [u8; 10])>,
        right: Option<(u8, [u8; 10])>,
    ) -> Result<()> {
        match self.request(DaemonCommand::SetTriggerEffects { left, right })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_vibration(&mut self, rumble: u8, trigger: u8) -> Result<()> {
        match self.request(DaemonCommand::SetVibration { rumble, trigger })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_speaker(&mut self, mode: &str) -> Result<()> {
        match self.request(DaemonCommand::SetSpeaker {
            mode: mode.to_string(),
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_volume(&mut self, volume: u8) -> Result<()> {
        match self.request(DaemonCommand::SetVolume { volume })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }
    pub fn set_update_mode(&mut self, active: bool) -> Result<()> {
        self.request(DaemonCommand::SetUpdateMode { active })
            .map(|_| ())
    }

    pub fn set_input_transform(&mut self, transform: InputTransform) -> Result<()> {
        match self.request(DaemonCommand::SetInputTransform { transform })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn clear_input_transform(&mut self) -> Result<()> {
        match self.request(DaemonCommand::ClearInputTransform)? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_lightbar_effect(&mut self, effect: LightbarEffect) -> Result<()> {
        match self.request(DaemonCommand::SetLightbarEffect { effect })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_haptic_pattern(
        &mut self,
        pattern: HapticPattern,
        strength: u8,
        speed: f32,
    ) -> Result<()> {
        match self.request(DaemonCommand::SetHapticPattern {
            pattern,
            strength,
            speed,
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn set_gyro(&mut self, enabled: bool, smoothing: f32, sensitivity: f32) -> Result<()> {
        match self.request(DaemonCommand::SetGyro {
            enabled,
            smoothing,
            sensitivity,
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn switch_profile(&mut self, name: &str) -> Result<()> {
        match self.request(DaemonCommand::SwitchProfile {
            name: name.to_string(),
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn reload_profile(&mut self) -> Result<()> {
        match self.request(DaemonCommand::ReloadProfile)? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn list_profiles(&mut self) -> Result<Vec<String>> {
        match self.request(DaemonCommand::ListProfiles)? {
            DaemonResponse::ProfileList { profiles } => Ok(profiles),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }

    pub fn save_profile(&mut self, profile: Profile) -> Result<()> {
        match self.request(DaemonCommand::SaveProfile { profile })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn delete_profile(&mut self, name: &str) -> Result<()> {
        match self.request(DaemonCommand::DeleteProfile {
            name: name.to_string(),
        })? {
            DaemonResponse::Ok => Ok(()),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => Ok(()),
        }
    }

    pub fn get_active_profile(&mut self) -> Result<String> {
        match self.request(DaemonCommand::GetActiveProfile)? {
            DaemonResponse::ActiveProfile { name } => Ok(name),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response"),
        }
    }
}
