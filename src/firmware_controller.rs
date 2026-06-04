use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Receiver;
use std::thread::JoinHandle;

use crate::firmware::FirmwareDownloader;
use crate::state::ProgressUpdate;

pub(crate) struct FirmwareController {
    pub(crate) downloader: FirmwareDownloader,
    pub(crate) progress_rx: Option<Receiver<ProgressUpdate>>,
    pub(crate) thread: Option<JoinHandle<()>>,

    pub(crate) progress: u32,
    pub(crate) status: String,
    pub(crate) updating: bool,
    pub(crate) used_daemon: bool,

    pub(crate) update_mode_flag: Option<Arc<AtomicBool>>,

    pub(crate) current_version: Option<u16>,
    pub(crate) latest_version: Option<String>,
    pub(crate) checking_latest: bool,
    pub(crate) build_date: Option<String>,
    pub(crate) build_time: Option<String>,
}

impl FirmwareController {
    pub(crate) fn new() -> Self {
        Self {
            downloader: FirmwareDownloader::new(),
            progress_rx: None,
            thread: None,

            progress: 0,
            status: String::new(),
            updating: false,
            used_daemon: false,

            update_mode_flag: None,

            current_version: None,
            latest_version: None,
            checking_latest: false,
            build_date: None,
            build_time: None,
        }
    }

    pub(crate) fn reap_thread(&mut self) {
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
    }
}

impl Drop for FirmwareController {
    fn drop(&mut self) {
        if let Some(h) = self.thread.take() {
            eprintln!("[ds4u] waiting for firmware thread to finish on shutdown...");
            let _ = h.join();
        }
    }
}
