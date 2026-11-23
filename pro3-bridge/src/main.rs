#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod bridge;
mod config;
mod dsu;
mod input_sim;
mod protocol;
mod setup;
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
use ui::Pro3App;

/// Dedicated square application icon (the .exe icon is the same artwork, see build.rs).
const APP_ICON_SVG: &[u8] = include_bytes!("../../assets/app_icon.svg");

/// Renders the embedded app icon SVG into a square RGBA icon (for window taskbar + system tray).
fn render_square_icon(size: u32) -> Option<Vec<u8>> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(APP_ICON_SVG, &opt).ok()?;
    let tw = tree.size().width();
    let th = tree.size().height();
    let scale = (size as f32 / tw).min(size as f32 / th);
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

/// Uses native Windows typography (Segoe UI + tabular Cascadia Mono / Consolas) when available,
/// with Segoe UI Symbol as a glyph fallback (arrows etc.). Falls back to egui's bundled fonts.
fn install_native_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions, FontFamily};
    let fonts_dir = std::path::PathBuf::from(
        std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()),
    )
    .join("Fonts");
    let load = |file: &str| std::fs::read(fonts_dir.join(file)).ok();

    let mut fonts = FontDefinitions::default();
    let mut add = |name: &str, bytes: Vec<u8>, families: &[(FontFamily, bool)]| {
        fonts
            .font_data
            .insert(name.to_owned(), Arc::new(FontData::from_owned(bytes)));
        for (family, front) in families {
            let list = fonts.families.entry(family.clone()).or_default();
            if *front {
                list.insert(0, name.to_owned());
            } else {
                list.push(name.to_owned());
            }
        }
    };

    if let Some(b) = load("segoeui.ttf") {
        add("segoe_ui", b, &[(FontFamily::Proportional, true)]);
    }
    if let Some(b) = load("CascadiaMono.ttf").or_else(|| load("consola.ttf")) {
        add("native_mono", b, &[(FontFamily::Monospace, true)]);
    }
    if let Some(b) = load("seguisym.ttf") {
        add(
            "segoe_symbol",
            b,
            &[(FontFamily::Proportional, false), (FontFamily::Monospace, false)],
        );
    }
    ctx.set_fonts(fonts);
}

/// Flagship dark theme: matte canvas, soft surfaces, hairline strokes, refined type scale.
fn apply_theme(ctx: &egui::Context) {
    use egui::{Color32 as C, CornerRadius, FontId, Shadow, Stroke, TextStyle};

    ctx.set_theme(egui::ThemePreference::Dark);
    install_native_fonts(ctx);
    let mut style = (*ctx.style()).clone();

    style.text_styles.insert(TextStyle::Heading, FontId::proportional(17.0));
    style.text_styles.insert(TextStyle::Body, FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Button, FontId::proportional(13.0));
    style.text_styles.insert(TextStyle::Small, FontId::proportional(11.0));
    style.text_styles.insert(TextStyle::Monospace, FontId::monospace(12.0));

    let sp = &mut style.spacing;
    sp.item_spacing = egui::vec2(8.0, 8.0);
    sp.button_padding = egui::vec2(14.0, 6.0);
    sp.interact_size = egui::vec2(40.0, 26.0);
    sp.slider_width = 200.0;
    sp.slider_rail_height = 4.0;
    sp.combo_width = 220.0;
    sp.menu_margin = egui::Margin::same(6);
    sp.scroll.bar_width = 6.0;
    sp.scroll.floating = true;

    let v = &mut style.visuals;
    v.dark_mode = true;
    v.panel_fill = ui::BG_CANVAS;
    v.window_fill = ui::BG_ELEVATED;
    v.window_stroke = Stroke::new(1.0, ui::STROKE_STRONG);
    v.window_corner_radius = CornerRadius::same(12);
    v.menu_corner_radius = CornerRadius::same(10);
    v.window_shadow = Shadow {
        offset: [0, 12],
        blur: 32,
        spread: 0,
        color: C::from_black_alpha(130),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 18,
        spread: 0,
        color: C::from_black_alpha(110),
    };
    v.extreme_bg_color = ui::BG_INSET;
    v.faint_bg_color = C::from_rgb(17, 20, 30);
    v.code_bg_color = ui::BG_INSET;
    v.hyperlink_color = C::from_rgb(56, 189, 248);
    v.selection.bg_fill = C::from_rgb(22, 104, 150);
    v.selection.stroke = Stroke::new(1.0, C::from_rgb(125, 211, 252));
    v.slider_trailing_fill = true;
    v.handle_shape = egui::style::HandleShape::Circle;

    let cr = CornerRadius::same(8);
    let w = &mut v.widgets;
    w.noninteractive.bg_fill = ui::BG_CARD;
    w.noninteractive.weak_bg_fill = ui::BG_CARD;
    w.noninteractive.bg_stroke = Stroke::new(1.0, ui::STROKE);
    w.noninteractive.fg_stroke = Stroke::new(1.0, C::from_rgb(196, 203, 218));
    w.noninteractive.corner_radius = cr;

    w.inactive.bg_fill = ui::BG_RAISED;
    w.inactive.weak_bg_fill = C::from_rgb(25, 30, 43);
    w.inactive.bg_stroke = Stroke::new(1.0, C::from_rgb(38, 45, 63));
    w.inactive.fg_stroke = Stroke::new(1.0, C::from_rgb(216, 222, 234));
    w.inactive.corner_radius = cr;

    w.hovered.bg_fill = C::from_rgb(46, 55, 76);
    w.hovered.weak_bg_fill = C::from_rgb(32, 38, 55);
    w.hovered.bg_stroke = Stroke::new(1.0, C::from_rgb(64, 76, 104));
    w.hovered.fg_stroke = Stroke::new(1.5, C::from_rgb(242, 245, 250));
    w.hovered.corner_radius = cr;
    w.hovered.expansion = 0.0;

    w.active.bg_fill = C::from_rgb(56, 66, 92);
    w.active.weak_bg_fill = C::from_rgb(38, 46, 66);
    w.active.bg_stroke = Stroke::new(1.0, C::from_rgb(56, 189, 248));
    w.active.fg_stroke = Stroke::new(1.5, C::WHITE);
    w.active.corner_radius = cr;
    w.active.expansion = 0.0;

    w.open.bg_fill = C::from_rgb(25, 30, 43);
    w.open.weak_bg_fill = C::from_rgb(25, 30, 43);
    w.open.bg_stroke = Stroke::new(1.0, ui::STROKE_STRONG);
    w.open.corner_radius = cr;

    ctx.set_style(style);
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
        .with_inner_size([880.0, 640.0])
        .with_min_inner_size([780.0, 580.0]);
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
            apply_theme(&cc.egui_ctx);

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
