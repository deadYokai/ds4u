use std::{env, io::{BufRead, BufReader, Write}, os::unix::net::UnixStream, path::{Path, PathBuf}, time::Duration};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

use crate::{common::MicLedState, dualsense::BatteryInfo, inputs::ControllerState};

pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(env::temp_dir)
        .join("ds4u.socket")
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "cmd", content = "args")]
pub enum DaemonCommand {
    Ping,
    GetBattery,
    GetInputState,
    GetFirmwareInfo,
    GetControllerInfo,
    SetLightbar { r: u8, g: u8, b: u8, brightness: u8 },
    SetLightbarEnabled { enabled: bool },
    SetPlayerLeds { leds: u8 },
    SetMic { enabled: bool },
    SetMicLed { state: MicLedState },
    SetTriggerOff,
    SetTriggerEffect { right: bool, left: bool, effect_type: u8, params: [u8; 10] },
    SetVibration { rumble: u8, trigger: u8 },
    SetSpeaker { mode: String },
    SetVolume { volume: u8 },
    SetUpdateMode { active: bool }
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum DaemonResponse {
    Pong,
    Ok,
    Error { message: String },
    Battery(BatteryInfo),
    InputState(ControllerState),
    FirmwareInfo { version: u16, build_date: String, build_time: String },
    ControllerInfo { serial: String, product_id: u16, is_bt: bool },
    NoDevice
}

pub struct IpcClient {
    pub socket_path: PathBuf,
    reader: BufReader<UnixStream>,
    writer: UnixStream
}

impl IpcClient {
    pub fn connect(path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let writer = stream.try_clone()?;

        Ok(Self{
            socket_path: path.to_owned(),
            reader: BufReader::new(stream),
            writer
        })
    }

    pub fn try_connect(path: &Path) -> Option<Self> {
        let mut c = Self::connect(path).ok()?;
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
            _ => bail!("Unexpected response")
        }
    }

    pub fn get_input_state(&mut self) -> Result<ControllerState> {
        match self.request(DaemonCommand::GetInputState)? {
            DaemonResponse::InputState(s) => Ok(s),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response")
        }        
    }

    pub fn get_firmware_info(&mut self) -> Result<(u16, String, String)> {
        match self.request(DaemonCommand::GetFirmwareInfo)? {
            DaemonResponse::FirmwareInfo { version, build_date, build_time } => 
                Ok((version, build_date, build_time)),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response")
        }         
    }

    pub fn get_controller_info(&mut self) -> Result<Option<(String, u16, bool)>> {
        match self.request(DaemonCommand::GetControllerInfo)? {
            DaemonResponse::ControllerInfo { serial, product_id, is_bt } => 
                Ok(Some((serial, product_id, is_bt))),
            DaemonResponse::Error { message } => bail!("{}", message),
            _ => bail!("Unexpected response")
        }         
    }

    pub fn set_lightbar(&mut self, r: u8, g: u8, b: u8, brightness: u8) -> Result<()> {
        self.request(DaemonCommand::SetLightbar { r, g, b, brightness }).map(|_| ())
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
        match self.request(
            DaemonCommand::SetTriggerEffect { right, left, effect_type, params })?
        {
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
        match self.request(DaemonCommand::SetSpeaker { mode: mode.to_string() })? {
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
        self.request(DaemonCommand::SetUpdateMode { active }).map(|_| ())
    }
}

