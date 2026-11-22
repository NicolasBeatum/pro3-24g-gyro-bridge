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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tray_icon::menu::{Menu, MenuEvent, MenuItem};
use tray_icon::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use ui::{Pro3App, PRO3_SVG};

/// Renders the embedded 8BitDo Pro 3 SVG into a square RGBA icon (for window taskbar + system tray).
fn render_square_icon(size: u32) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(PRO3_SVG, &opt).ok()?;
    let tw = tree.size().width();
    let th = tree.size().height();
    let scale = (size as f32 / tw).min(size as f32 / th) * 0.96;
    let dx = (size as f32 - tw * scale) * 0.5;
    let dy = (size as f32 - th * scale) * 0.5;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    let tf = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(dx, dy);
    resvg::render(&tree, tf, &mut pixmap.as_mut());
    // Un-premultiply alpha for standard RGBA icons
    let mut rgba = pixmap.take();
    for px in rgba.chunks_exact_mut(4) {
        let a = px[3] as u32;
        if a > 0 && a < 255 {
            px[0] = ((px[0] as u32 * 255) / a).min(255) as u8;
            px[1] = ((px[1] as u32 * 255) / a).min(255) as u8;
            px[2] = ((px[2] as u32 * 255) / a).min(255) as u8;
        }
    }
    Some(rgba)
}

fn create_tray_icon(icon_rgba: Option<&[u8]>) -> Option<(TrayIcon, String, String)> {
    let menu = Menu::new();
    let show_item = MenuItem::new("Mostrar / Show", true, None);
    let quit_item = MenuItem::new("Salir / Exit", true, None);
    let show_id = show_item.id().0.clone();
    let quit_id = quit_item.id().0.clone();
    let _ = menu.append(&show_item);
    let _ = menu.append(&quit_item);

    let icon = if let Some(rgba) = icon_rgba {
        tray_icon::Icon::from_rgba(rgba.to_vec(), 32, 32).ok()?
    } else {
        tray_icon::Icon::from_rgba(vec![88, 90, 190, 255].repeat(32 * 32), 32, 32).ok()?
    };

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("8BitDo Pro 3 · 2.4 GHz Gyro Bridge (200 Hz)")
        .with_icon(icon)
        .build()
        .ok()?;
    Some((tray, show_id, quit_id))
}

fn main() -> eframe::Result<()> {
    let cfg = BridgeConfig::load();
    let ctrl = BridgeControl::new(cfg.clone());
    let worker = spawn_bridge_thread(ctrl.clone());

    let icon_rgba_32 = render_square_icon(32);
    let icon_rgba_64 = render_square_icon(64);

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("8BitDo Pro 3 · 2.4 GHz Gyro Bridge")
        .with_inner_size([860.0, 650.0])
        .with_min_inner_size([740.0, 560.0]);
    if let Some(rgba) = icon_rgba_64 {
        viewport = viewport.with_icon(Arc::new(egui::IconData {
            rgba,
            width: 64,
            height: 64,
        }));
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let force_exit = Arc::new(AtomicBool::new(false));
    let force_exit_app = force_exit.clone();
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
            visuals.selection.stroke =
                egui::Stroke::new(1.0, egui::Color32::from_rgb(112, 178, 255));
            cc.egui_ctx.set_visuals(visuals);

            // Create Windows system tray icon on UI thread (where Windows message loop runs)
            let tray_handle = create_tray_icon(icon_rgba_32.as_deref());
            let (tray_keepalive, show_id, quit_id) = match tray_handle {
                Some((t, s, q)) => (Some(t), s, q),
                None => (None, String::new(), String::new()),
            };

            // Lightweight background listener so tray clicks ALWAYS wake a hidden window immediately
            let ctx_tray = cc.egui_ctx.clone();
            let force_exit_tray = force_exit.clone();
            let shutdown_tray = ctrl.clone();
            std::thread::Builder::new()
                .name("tray-events".into())
                .spawn(move || {
                    let tray_rx = TrayIconEvent::receiver();
                    let menu_rx = MenuEvent::receiver();
                    while !shutdown_tray.shutdown.load(Ordering::Relaxed) {
                        while let Ok(ev) = tray_rx.try_recv() {
                            let is_open = matches!(
                                ev,
                                TrayIconEvent::Click {
                                    button: MouseButton::Left,
                                    button_state: MouseButtonState::Up,
                                    ..
                                } | TrayIconEvent::DoubleClick {
                                    button: MouseButton::Left,
                                    ..
                                }
                            );
                            if is_open {
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Focus);
                                ctx_tray.request_repaint();
                            }
                        }
                        while let Ok(ev) = menu_rx.try_recv() {
                            if ev.id.0 == show_id {
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Focus);
                                ctx_tray.request_repaint();
                            } else if ev.id.0 == quit_id {
                                force_exit_tray.store(true, Ordering::Relaxed);
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                                ctx_tray.send_viewport_cmd(egui::ViewportCommand::Close);
                                ctx_tray.request_repaint();
                            }
                        }
                        std::thread::sleep(Duration::from_millis(40));
                    }
                })
                .ok();

            Ok(Box::new(Pro3App::new(
                ctrl,
                cfg,
                tray_keepalive,
                force_exit_app,
            )))
        }),
    );

    ctrl_close.shutdown.store(true, Ordering::Relaxed);
    let _ = worker.join();
    res
}
