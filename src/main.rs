use std::env;

use crate::app::DS4UApp;

use self::ipc::IpcClient;

mod app;
mod backend;
mod common;
mod daemon;
mod dualsense;
mod firmware;
mod firmware_controller;
mod input_poller;
mod inputs;
mod ipc;
mod profiles;
mod settings;
mod state;
mod style;
mod theme;
mod transform;
mod ui;
mod util;

fn main() -> Result<(), eframe::Error> {
    let args: Vec<String> = env::args().collect();

    if args.iter().any(|a| a == "--daemon") {
        daemon::run_daemon();
        return Ok(());
    }

    if args.len() >= 2 {
        let addr = ipc::daemon_endpoint();
        let mut client = match IpcClient::try_connect(&addr) {
            Some(c) => c,
            None => {
                eprintln!("ds4u daemon is not running");
                std::process::exit(1);
            }
        };

        match args[1].as_str() {
            "--list-profiles" => {
                match client.list_profiles() {
                    Ok(list) => {
                        for name in list {
                            println!("{}", name);
                        }
                    }
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            "--switch-profile" => {
                let name = args.get(2).map(|s| s.as_str()).unwrap_or_else(|| {
                    eprintln!("usage: ds4u --switch-profile <name>");
                    std::process::exit(1);
                });
                match client.switch_profile(name) {
                    Ok(_) => println!("switched to profile '{}'", name),
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            "--reload-profile" => {
                match client.reload_profile() {
                    Ok(_) => println!("profile reloaded"),
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                }
                return Ok(());
            }
            "--status" => {
                match client.get_controller_info() {
                    Ok(Some((serial, pid, is_bt))) => println!(
                        "connected  serial={}  pid={:#06x}  {}",
                        serial,
                        pid,
                        if is_bt { "bluetooth" } else { "usb" }
                    ),
                    Ok(None) => println!("no device"),
                    Err(e) => {
                        eprintln!("error: {}", e);
                        std::process::exit(1);
                    }
                }
                if let Ok(b) = client.get_battery() {
                    println!("battery    {:?}", b)
                }
                return Ok(());
            }
            _ => {}
        }
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
        }),
    )
}
