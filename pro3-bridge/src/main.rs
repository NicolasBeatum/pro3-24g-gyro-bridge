#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod config;
mod dsu;
mod input_sim;
mod protocol;
mod ui;
mod vigem;

use bridge::{spawn_bridge_thread, BridgeControl};
use config::BridgeConfig;
use eframe::egui;
use std::sync::atomic::Ordering;
use ui::Pro3App;

fn main() -> eframe::Result<()> {
    let cfg = BridgeConfig::load();
    let ctrl = BridgeControl::new(cfg.clone());
    let worker = spawn_bridge_thread(ctrl.clone());

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("8BitDo Pro 3 · 2.4 GHz Gyro Bridge")
            .with_inner_size([740.0, 560.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    let ctrl_close = ctrl.clone();
    let res = eframe::run_native(
        "8BitDo Pro 3 · 2.4 GHz Gyro Bridge",
        native_options,
        Box::new(move |cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(Pro3App::new(ctrl, cfg)))
        }),
    );

    ctrl_close.shutdown.store(true, Ordering::Relaxed);
    let _ = worker.join();
    res
}
