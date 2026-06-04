use std::sync::mpsc::Receiver;
use std::sync::{Arc, atomic::AtomicBool};
use std::thread::JoinHandle;

use crate::inputs::ControllerState;

pub(crate) struct InputPoller {
    pub(crate) state_rx: Option<Receiver<ControllerState>>,
    pub(crate) stop: Option<Arc<AtomicBool>>,
    pub(crate) thread: Option<JoinHandle<()>>,
    pub(crate) polling: bool,
    pub(crate) controller_state: Option<ControllerState>,
}

impl InputPoller {
    pub(crate) fn new() -> Self {
        Self {
            state_rx: None,
            stop: None,
            thread: None,
            polling: false,
            controller_state: None,
        }
    }

    pub(crate) fn stop(&mut self) {
        if let Some(flag) = self.stop.take() {
            flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if let Some(h) = self.thread.take() {
            let _ = h.join();
        }
        self.state_rx = None;
        self.polling = false;
        self.controller_state = None;
    }
}

impl Drop for InputPoller {
    fn drop(&mut self) {
        self.stop();
    }
}
