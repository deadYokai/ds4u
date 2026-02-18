use std::io::Read;

use anyhow::{anyhow, bail, Result};
use serde::Deserialize;

use crate::common::*;

const FIRMWARE_BASE_URL: &str = "https://fwupdater.dl.playstation.net/fwupdater/";

#[derive(Deserialize)]
struct FirmwareInfo {
    #[serde(rename = "FwUpdate0004LatestVersion")]
    dualsense_version: Option<String>,
    #[serde(rename = "FwUpdate0044LatestVersion")]
    dualsense_edge_version: Option<String>,
}

#[derive(Clone)]
pub struct FirmwareDownloader {
    client: reqwest::blocking::Client
}

impl FirmwareDownloader {
    pub fn new() -> Self {
        Self { client: reqwest::blocking::Client::new() }
    }

    pub fn get_latest_version(&self) -> Result<(String, String)> {
        let url = format!("{}info.json", FIRMWARE_BASE_URL);
        let response = self.client.get(&url).send()?;
        let info: FirmwareInfo = response.json()?;

        let ds_version = info.dualsense_version
            .ok_or(anyhow!("DualSense version not found in info.json"))?;
        let ds_edge_version = info.dualsense_edge_version
            .ok_or(anyhow!("DualSense Edge version not found in info.json"))?;

        Ok((ds_version, ds_edge_version))
    }

    pub fn download_firmware(
        &self, pid: u16, version: &str, progress_callback: impl Fn(u32)
    ) -> Result<Vec<u8>> {
        let (fw_path, filename) = match pid {
            DS_PID => ("fwupdate0004", "FWUPDATE0004.bin"),
            DSE_PID => ("fwupdate0044", "FWUPDATE0044.bin"),
            _ => bail!("Unknown product ID")
        };

        let url = format!("{}{}/{}/{}",
            FIRMWARE_BASE_URL, fw_path, version, filename);

        let mut response = self.client.get(&url).send()
            .map_err(|e| anyhow!("Download failed: {}. Check internet connection.", e))?;

        if !response.status().is_success() {
            bail!("Donwload failed with status: {}", response.status());
        }

        println!("{:?}", url);

        let total_size = response.content_length().unwrap_or(FIRMWARE_SIZE as u64);

        let mut fw_data = Vec::with_capacity(FIRMWARE_SIZE);

        let mut buffer = [0u8; 8196];
        let mut downloaded: u64 = 0;

        progress_callback(0);

        loop {
            let bytes_read = response.read(&mut buffer)
                .map_err(|e| anyhow!("Download interrupted: {}", e))?;

            if bytes_read == 0 {
                break;
            }

            fw_data.extend_from_slice(&buffer[..bytes_read]);
            downloaded += bytes_read as u64;

            let progress = ((downloaded * 100) / total_size).min(100) as u32;
            progress_callback(progress);
        }

        progress_callback(100);

        Ok(fw_data)
    }

    pub fn download_latest_firmware(
        &self, pid: u16, progress_callback: impl Fn(u32)
    ) -> Result<Vec<u8>> {
        let (ds_version, ds_edge_version) = self.get_latest_version()?;

        let version = match pid {
            DS_PID => ds_version,
            DSE_PID => ds_edge_version,
            _ => bail!("Unknown product ID")
        };

        self.download_firmware(pid, &version, progress_callback)
    }
}

pub fn get_product_name(product_id: u16) -> &'static str {
    match product_id {
        DS_PID => "DualSense",
        DSE_PID => "DualSense Edge",
        _ => "Unknown",
    }
}

