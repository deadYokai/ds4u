use std::{sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex}, thread::{self, sleep}, time::Duration};

use anyhow::Result;
use hidapi::HidApi;

use crate::{
    dualsense::{self, DualSense},
    profiles::{Profile, ProfileManager},
    common::TriggerMode
};

pub struct DaemonManager {
    running: Arc<Mutex<bool>>,
    profile_manager: ProfileManager,
    auto_apply_enabled: Arc<Mutex<bool>>,
    current_profile: Arc<Mutex<Option<String>>>,
    update_in_progress: Arc<AtomicBool>
}

impl DaemonManager {
    pub fn new() -> Self {
        Self {
            running: Arc::new(Mutex::new(false)),
            profile_manager: ProfileManager::new(),
            auto_apply_enabled: Arc::new(Mutex::new(false)),
            current_profile: Arc::new(Mutex::new(None)),
            update_in_progress: Arc::new(AtomicBool::new(false))
        }
    }

    pub fn start(&mut self) -> Result<()> {
        let mut running = self.running.lock().unwrap();

        if *running {
            return Ok(());
        }

        *running = true;
        drop(running);

        let running_clone = Arc::clone(&self.running);
        let auto_apply = Arc::clone(&self.auto_apply_enabled);
        let current_profile = Arc::clone(&self.current_profile);
        let profile_manager = self.profile_manager.clone();
        let update_in_progres = Arc::clone(&self.update_in_progress);

        thread::spawn(move || {
            daemon_loop(
                running_clone,
                auto_apply,
                current_profile,
                profile_manager,
                update_in_progres
            );
        });

        Ok(())
    }

    pub fn stop(&mut self) {
        let mut running = self.running.lock().unwrap();
        *running = false;
    }

    pub fn is_running(&self) -> bool {
        *self.running.lock().unwrap()
    }

    pub fn set_auto_apply(&mut self, enabled: bool) {
        let mut auto_apply = self.auto_apply_enabled.lock().unwrap();
        *auto_apply = enabled;
    }

    pub fn set_auto_profile(&mut self, profile_name: Option<String>) {
        let mut current = self.current_profile.lock().unwrap();
        *current = profile_name;
    }

    pub fn set_update_in_progress(&self, active: bool) {
        self.update_in_progress.store(active, Ordering::SeqCst);
    }
}

fn daemon_loop(
    running: Arc<Mutex<bool>>,
    auto_apply_enabled: Arc<Mutex<bool>>,
    current_profile: Arc<Mutex<Option<String>>>,
    profile_manager: ProfileManager,
    update_in_progress: Arc<AtomicBool>
) {
    let mut last_connected = false;

    while *running.lock().unwrap() {
        if update_in_progress.load(Ordering::Relaxed) {
            sleep(Duration::from_millis(500));
            continue;
        }

        if let Ok(api) = HidApi::new() {
            let connected = !dualsense::list_devices(&api).is_empty();

            if connected && !last_connected 
                && *auto_apply_enabled.lock().unwrap() 
                && let Some(profile_name) = &*current_profile.lock().unwrap() 
                && let Ok(profile) = profile_manager.load_profile(profile_name) 
                && !update_in_progress.load(Ordering::Relaxed)
            {
                        let _ = apply_profile_to_controller(&api, &profile);
                        println!("Applied profile: {}", profile_name);
            }

            last_connected = connected;
        }

        sleep(Duration::from_secs(2));
    }
}

fn apply_profile_to_controller(api: &HidApi, profile: &Profile) -> Result<()> {
    if let Ok(mut controller) = DualSense::new(api, None) {
        let _ = controller.set_lightbar(
            (profile.lightbar_r * 255.0) as u8,
            (profile.lightbar_g * 255.0) as u8,
            (profile.lightbar_b * 255.0) as u8,
            profile.lightbar_brightness as u8,
        );

        let _ = controller.set_player_leds(profile.player_leds);

        let _ = controller.set_mic(profile.mic_enabled);

        if profile.trigger_mode == TriggerMode::Off {
            let _ = controller.set_trigger_off();
        }
    }

    Ok(())
}
