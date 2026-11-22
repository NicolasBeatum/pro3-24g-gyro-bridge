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
            .with_inner_size([840.0, 640.0])
            .with_min_inner_size([740.0, 560.0]),
        ..Default::default()
    };

    let ctrl_close = ctrl.clone();
    let res = eframe::run_native(
        "8BitDo Pro 3 · 2.4 GHz Gyro Bridge",
        native_options,
        Box::new(move |cc| {
            let mut visuals = egui::Visuals::dark();
            visuals.panel_fill = egui::Color32::from_rgb(11, 13, 19);
            visuals.window_fill = egui::Color32::from_rgb(18, 21, 30);
            visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(22, 26, 36);
            visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(28, 33, 46);
            visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(38, 45, 62);
            visuals.widgets.active.bg_fill = egui::Color32::from_rgb(48, 92, 178);
            visuals.selection.bg_fill = egui::Color32::from_rgb(46, 108, 214);
            visuals.selection.stroke = egui::Stroke::new(1.0, egui::Color32::from_rgb(112, 178, 255));
            cc.egui_ctx.set_visuals(visuals);
            Ok(Box::new(Pro3App::new(ctrl, cfg)))
        }),
    );

    ctrl_close.shutdown.store(true, Ordering::Relaxed);
    let _ = worker.join();
    res
}
