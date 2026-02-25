use std::env;

use crate::app::DS4UApp;

mod dualsense;
mod state;
mod firmware;
mod profiles;
mod daemon;
mod common;
mod inputs;
mod ipc;
mod transform;
mod app;
mod style;
mod ui;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--daemon") {
        daemon::run_daemon();
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([1000.0, 700.0]),
        ..Default::default()
    };

    eframe::run_native(
        "DS4U",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            Ok(Box::new(DS4UApp::new()))
        })
    )
}


