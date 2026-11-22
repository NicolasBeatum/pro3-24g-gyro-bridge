use crate::bridge::{BridgeControl, BridgeStatus};
use crate::config::*;
use crate::protocol::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tray_icon::TrayIcon;

pub const PRO3_SVG: &[u8] = include_bytes!("../../assets/8bitdo_pro3.svg");

// Sleek obsidian / slate palette
const BG_CANVAS: Color32 = Color32::from_rgb(11, 13, 19);
const BG_HEADER: Color32 = Color32::from_rgb(16, 19, 28);
const BG_CARD: Color32 = Color32::from_rgb(20, 24, 35);
const BORDER_CARD: Color32 = Color32::from_rgb(38, 45, 64);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(236, 241, 250);
const TEXT_MUTED: Color32 = Color32::from_rgb(142, 152, 174);
const ACCENT_CYAN: Color32 = Color32::from_rgb(56, 189, 248);
const ACCENT_GREEN: Color32 = Color32::from_rgb(52, 211, 153);
const ACCENT_CORAL: Color32 = Color32::from_rgb(251, 113, 133);
const ACCENT_AMBER: Color32 = Color32::from_rgb(251, 191, 36);
const ACCENT_VIOLET: Color32 = Color32::from_rgb(129, 140, 248);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Dashboard,
    Paddles,
    Advanced,
}

pub struct Pro3App {
    ctrl: Arc<BridgeControl>,
    cfg: BridgeConfig,
    tab: Tab,
    pro3_tex: Option<egui::TextureHandle>,
    tex_loaded: bool,
    _tray: Option<TrayIcon>,
    force_exit: Arc<AtomicBool>,
    show_close_modal: bool,
    remember_close_choice: bool,
}

impl Pro3App {
    pub fn new(
        ctrl: Arc<BridgeControl>,
        cfg: BridgeConfig,
        tray: Option<TrayIcon>,
        force_exit: Arc<AtomicBool>,
    ) -> Self {
        Self {
            ctrl,
            cfg,
            tab: Tab::Dashboard,
            pro3_tex: None,
            tex_loaded: false,
            _tray: tray,
            force_exit,
            show_close_modal: false,
            remember_close_choice: true,
        }
    }

    fn tr(&self, es: &'static str, en: &'static str) -> &'static str {
        match self.cfg.lang {
            Language::Es => es,
            Language::En => en,
        }
    }

    fn ensure_texture(&mut self, ctx: &egui::Context) {
        if self.tex_loaded {
            return;
        }
        self.tex_loaded = true;
        let mut opt = resvg::usvg::Options::default();
        opt.fontdb_mut().load_system_fonts();
        if let Ok(tree) = resvg::usvg::Tree::from_data(PRO3_SVG, &opt) {
            let scale = 2.0f32;
            let w = (tree.size().width() * scale).round() as u32;
            let h = (tree.size().height() * scale).round() as u32;
            if let Some(mut pixmap) = resvg::tiny_skia::Pixmap::new(w, h) {
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::from_scale(scale, scale),
                    &mut pixmap.as_mut(),
                );
                let img = egui::ColorImage::from_rgba_premultiplied(
                    [w as usize, h as usize],
                    pixmap.data(),
                );
                self.pro3_tex =
                    Some(ctx.load_texture("pro3_svg", img, egui::TextureOptions::LINEAR));
            }
        }
    }

    fn sync_config(&self) {
        if let Ok(mut c) = self.ctrl.config.lock() {
            *c = self.cfg.clone();
        }
        self.cfg.save();
    }

    fn handle_close_request(&mut self, ctx: &egui::Context) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.force_exit.load(Ordering::Relaxed)
        {
            match self.cfg.close_action {
                CloseAction::Ask => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    self.show_close_modal = true;
                }
                CloseAction::MinimizeToTray => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                }
                CloseAction::Exit => {
                    // Allow normal close
                }
            }
        }
    }

    fn render_close_modal(&mut self, ctx: &egui::Context) {
        if !self.show_close_modal {
            return;
        }
        let title = self.tr("¿Qué querés hacer al cerrar?", "What would you like to do?");
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
            .default_width(440.0)
            .max_width(440.0)
            .frame(
                egui::Frame::window(&ctx.style())
                    .fill(Color32::from_rgb(20, 24, 36))
                    .stroke(Stroke::new(1.5, Color32::from_rgb(70, 96, 152)))
                    .corner_radius(CornerRadius::same(12))
                    .inner_margin(egui::Margin::same(18)),
            )
            .show(ctx, |ui| {
                ui.label(
                    RichText::new(self.tr(
                        "• Minimizar a la bandeja: oculta la ventana abajo junto al reloj y mantiene el giroscopio, Steam y DSU funcionando en segundo plano.",
                        "• Minimize to Tray: hides the window to the taskbar tray and keeps Gyro, Steam DS4, and DSU running in the background.",
                    ))
                    .size(12.5)
                    .color(TEXT_PRIMARY),
                );
                ui.add_space(6.0);
                ui.label(
                    RichText::new(self.tr(
                        "• Cerrar completamente: detiene la app. Deberás desenchufar y volver a enchufar el dongle USB para que el mando vuelva a funcionar en modo XInput normal.",
                        "• Exit Completely: stops the bridge. You will need to unplug and replug the USB dongle to restore default XInput mode.",
                    ))
                    .size(12.5)
                    .color(ACCENT_AMBER),
                );

                ui.add_space(12.0);
                let remember_lbl = self.tr(
                    "Recordar mi elección (se puede cambiar en Avanzado)",
                    "Remember my choice (can be changed in Advanced)",
                );
                ui.checkbox(&mut self.remember_close_choice, remember_lbl);

                ui.add_space(14.0);
                ui.horizontal(|ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(self.tr("Minimizar a la bandeja", "Minimize to Tray"))
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(34, 118, 210))
                            .corner_radius(CornerRadius::same(8))
                            .min_size(Vec2::new(160.0, 32.0)),
                        )
                        .clicked()
                    {
                        if self.remember_close_choice {
                            self.cfg.close_action = CloseAction::MinimizeToTray;
                            self.sync_config();
                        }
                        self.show_close_modal = false;
                        ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                    }

                    if ui
                        .add(
                            egui::Button::new(
                                RichText::new(self.tr("Cerrar app", "Exit App"))
                                    .strong()
                                    .color(Color32::WHITE),
                            )
                            .fill(Color32::from_rgb(185, 52, 68))
                            .corner_radius(CornerRadius::same(8))
                            .min_size(Vec2::new(105.0, 32.0)),
                        )
                        .clicked()
                    {
                        if self.remember_close_choice {
                            self.cfg.close_action = CloseAction::Exit;
                            self.sync_config();
                        }
                        self.show_close_modal = false;
                        self.force_exit.store(true, Ordering::Relaxed);
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }

                    if ui
                        .add(
                            egui::Button::new(self.tr("Cancelar", "Cancel"))
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(82.0, 32.0)),
                        )
                        .clicked()
                    {
                        self.show_close_modal = false;
                    }
                });
            });
    }
}

impl eframe::App for Pro3App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ensure_texture(ctx);
        self.handle_close_request(ctx);
        ctx.request_repaint_after(Duration::from_millis(16));

        let status = self
            .ctrl
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();
        let lang = self.cfg.lang;
        let prev_cfg_header = self.cfg.clone();

        // Top Header Bar
        egui::TopBottomPanel::top("header")
            .frame(
                egui::Frame::default()
                    .fill(BG_HEADER)
                    .stroke(Stroke::new(1.0, Color32::from_rgb(30, 35, 50)))
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new("8BitDo Pro 3")
                                    .size(20.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            egui::Frame::default()
                                .fill(Color32::from_rgb(24, 52, 78))
                                .stroke(Stroke::new(1.0, Color32::from_rgb(46, 118, 178)))
                                .corner_radius(CornerRadius::same(6))
                                .inner_margin(egui::Margin::symmetric(7, 2))
                                .show(ui, |ui| {
                                    ui.label(
                                        RichText::new("2.4 GHz · 200 Hz")
                                            .size(11.0)
                                            .strong()
                                            .color(ACCENT_CYAN),
                                    );
                                });
                        });

                        let dot_color = if status.connected {
                            ACCENT_GREEN
                        } else if status.running {
                            ACCENT_AMBER
                        } else {
                            TEXT_MUTED
                        };
                        ui.horizontal(|ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                            ui.painter().circle_filled(
                                rect.center(),
                                5.5,
                                Color32::from_rgba_unmultiplied(
                                    dot_color.r(),
                                    dot_color.g(),
                                    dot_color.b(),
                                    55,
                                ),
                            );
                            ui.painter().circle_filled(rect.center(), 3.6, dot_color);
                            ui.label(
                                RichText::new(status.conn_state.label(lang))
                                    .size(12.5)
                                    .color(Color32::from_rgb(176, 186, 206)),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Language Switcher ES | EN
                        ui.horizontal(|ui| {
                            ui.selectable_value(&mut self.cfg.lang, Language::En, "EN");
                            ui.selectable_value(&mut self.cfg.lang, Language::Es, "ES");
                        });

                        ui.add_space(6.0);

                        let is_running = self.ctrl.running.load(Ordering::Relaxed);
                        let btn_text = if is_running {
                            self.tr("Detener", "Stop")
                        } else {
                            self.tr("Iniciar", "Start")
                        };
                        let btn_fill = if is_running {
                            Color32::from_rgb(185, 52, 68)
                        } else {
                            Color32::from_rgb(28, 148, 96)
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(btn_text)
                                        .strong()
                                        .size(13.0)
                                        .color(Color32::WHITE),
                                )
                                .fill(btn_fill)
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(82.0, 32.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.running.store(!is_running, Ordering::Relaxed);
                        }

                        if ui
                            .add_enabled(
                                status.connected,
                                egui::Button::new(
                                    RichText::new(self.tr("Vibrar", "Rumble")).size(12.5),
                                )
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(72.0, 32.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.request_test_rumble.store(true, Ordering::Relaxed);
                        }

                        if ui
                            .add_enabled(
                                status.connected,
                                egui::Button::new(
                                    RichText::new(self.tr("Calibrar", "Calibrate")).size(12.5),
                                )
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(84.0, 32.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.request_calibrate.store(true, Ordering::Relaxed);
                        }
                    });
                });

                ui.add_space(9.0);
                ui.horizontal_wrapped(|ui| {
                    status_pill(
                        ui,
                        &format!("{:.0} Hz", status.rate_hz),
                        if status.rate_hz > 150.0 {
                            Color32::from_rgb(20, 74, 58)
                        } else {
                            Color32::from_rgb(34, 39, 52)
                        },
                        if status.rate_hz > 150.0 {
                            ACCENT_GREEN
                        } else {
                            TEXT_MUTED
                        },
                    );
                    let bat_lbl = match lang {
                        Language::Es => format!("Batería {}%", status.battery),
                        Language::En => format!("Battery {}%", status.battery),
                    };
                    status_pill(
                        ui,
                        &bat_lbl,
                        if status.battery >= 25 {
                            Color32::from_rgb(24, 50, 78)
                        } else {
                            Color32::from_rgb(104, 38, 42)
                        },
                        if status.battery >= 25 {
                            ACCENT_CYAN
                        } else {
                            ACCENT_CORAL
                        },
                    );
                    let vigem_lbl = if let Some(err) = &status.vigem_error {
                        err.clone()
                    } else {
                        match (lang, status.vigem_mode_active) {
                            (Language::Es, Some(PadMode::Ds4NativeGyro)) => {
                                "DualShock 4 (Gyro Nativo Steam)".into()
                            }
                            (Language::En, Some(PadMode::Ds4NativeGyro)) => {
                                "DualShock 4 (Native Steam Gyro)".into()
                            }
                            (_, Some(PadMode::Xbox360)) => "Xbox 360 (XInput)".into(),
                            (Language::Es, Some(PadMode::None)) => "Solo DSU".into(),
                            (Language::En, Some(PadMode::None)) => "DSU Only".into(),
                            (Language::Es, None) => "ViGEmBus inactivo".into(),
                            (Language::En, None) => "ViGEmBus inactive".into(),
                        }
                    };
                    status_pill(
                        ui,
                        &vigem_lbl,
                        if status.vigem_ok {
                            Color32::from_rgb(38, 42, 92)
                        } else {
                            Color32::from_rgb(62, 46, 38)
                        },
                        if status.vigem_ok {
                            ACCENT_VIOLET
                        } else {
                            ACCENT_AMBER
                        },
                    );
                    let dsu_txt = if self.cfg.dsu_enabled {
                        if let Some(err) = &status.dsu_error {
                            format!("DSU {err}")
                        } else {
                            format!("DSU :{} ({} cli)", self.cfg.dsu_port, status.dsu_clients)
                        }
                    } else {
                        "DSU: OFF".into()
                    };
                    status_pill(
                        ui,
                        &dsu_txt,
                        Color32::from_rgb(32, 37, 50),
                        Color32::from_rgb(190, 200, 220),
                    );
                });
            });

        if self.cfg != prev_cfg_header {
            self.sync_config();
        }

        // Sleek Navigation Tabs
        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::default()
                    .fill(Color32::from_rgb(14, 17, 24))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(28, 33, 46)))
                    .inner_margin(egui::Margin::symmetric(18, 7)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    let t1 = self.tr("Panel en Vivo", "Live Dashboard");
                    let t2 = self.tr("Botones Traseros (PL / PR)", "Back Paddles (PL / PR)");
                    let t3 = self.tr("Gyro PC & Avanzado", "PC Gyro & Advanced");
                    tab_button(ui, &mut self.tab, Tab::Dashboard, t1);
                    tab_button(ui, &mut self.tab, Tab::Paddles, t2);
                    tab_button(ui, &mut self.tab, Tab::Advanced, t3);
                });
            });

        // Main Content
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG_CANVAS)
                    .inner_margin(egui::Margin::same(16)),
            )
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let prev_cfg = self.cfg.clone();
                    match self.tab {
                        Tab::Dashboard => self.ui_dashboard(ui, &status),
                        Tab::Paddles => self.ui_paddles(ui, &status),
                        Tab::Advanced => self.ui_advanced(ui, &status),
                    }
                    if self.cfg != prev_cfg {
                        self.sync_config();
                    }
                });
            });

        self.render_close_modal(ctx);
    }
}

impl Pro3App {
    fn ui_dashboard(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let st = &status.telemetry;
        let lang = self.cfg.lang;

        // Top compact output bar
        let card_title = self.tr("Perfil de Salida", "Output Profile");
        card(ui, card_title, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(match lang { Language::Es => "Mando:", Language::En => "Controller:" }).color(TEXT_MUTED));
                ui.selectable_value(
                    &mut self.cfg.pad_mode,
                    PadMode::Ds4NativeGyro,
                    match lang {
                        Language::Es => "DualShock 4 (Gyro Nativo Steam)",
                        Language::En => "DualShock 4 (Native Steam Gyro)",
                    },
                );
                ui.selectable_value(&mut self.cfg.pad_mode, PadMode::Xbox360, "Xbox 360");
                ui.selectable_value(
                    &mut self.cfg.pad_mode,
                    PadMode::None,
                    match lang { Language::Es => "Solo DSU", Language::En => "DSU Only" },
                );

                ui.separator();
                ui.label(RichText::new(match lang { Language::Es => "Botones:", Language::En => "Buttons:" }).color(TEXT_MUTED));
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::XboxPs, "Xbox / PS");
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::Nintendo, "Nintendo");
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(
                    &mut self.cfg.dsu_enabled,
                    match lang {
                        Language::Es => "DSU Emuladores (UDP)",
                        Language::En => "DSU Emulators (UDP)",
                    },
                );
                ui.separator();
                ui.checkbox(
                    &mut self.cfg.rumble_enabled,
                    match lang { Language::Es => "Vibración", Language::En => "Rumble" },
                );
                if self.cfg.rumble_enabled {
                    ui.add(
                        egui::Slider::new(&mut self.cfg.rumble_strength, 0.1..=1.0)
                            .fixed_decimals(0)
                            .custom_formatter(|v, _| format!("{}%", (v * 100.0).round() as i32)),
                    );
                }
            });
        });

        ui.add_space(12.0);

        // Main visual showcase: Left = Interactive 8BitDo Pro 3 SVG Graphic, Right = Live Gyro Horizon + Telemetry
        ui.horizontal_top(|ui| {
            let total_w = ui.available_width();
            let left_w = (total_w * 0.58).max(420.0);
            let right_w = (total_w - left_w - 12.0).max(240.0);

            ui.allocate_ui_with_layout(
                Vec2::new(left_w, 355.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    card(
                        ui,
                        match lang {
                            Language::Es => "8BitDo Pro 3 · Vista Interactiva en Vivo",
                            Language::En => "8BitDo Pro 3 · Live Interactive View",
                        },
                        |ui| {
                            draw_pro3_controller(
                                ui,
                                self.pro3_tex.as_ref(),
                                st,
                                status.rumble_l,
                                status.rumble_r,
                            );
                        },
                    );
                },
            );

            ui.add_space(12.0);

            ui.allocate_ui_with_layout(
                Vec2::new(right_w, 355.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    card(
                        ui,
                        match lang {
                            Language::Es => "Giroscopio y Orientación (200 Hz)",
                            Language::En => "Gyroscope & Orientation (200 Hz)",
                        },
                        |ui| {
                            let gx = st.gx_dps - status.gyro_bias[0];
                            let gy = st.gy_dps - status.gyro_bias[1];
                            let gz = st.gz_dps - status.gyro_bias[2];
                            let pitch_dps = -gy;
                            let yaw_dps = gz;
                            let roll_dps = -gx;

                            draw_horizon_gauge(ui, -st.ay_g, st.az_g, -st.ax_g, yaw_dps);
                            ui.add_space(10.0);

                            draw_signed_bar(ui, "Pitch", pitch_dps, 250.0, "°/s", ACCENT_CYAN);
                            ui.add_space(5.0);
                            draw_signed_bar(ui, "Yaw  ", yaw_dps, 250.0, "°/s", ACCENT_VIOLET);
                            ui.add_space(5.0);
                            draw_signed_bar(ui, "Roll ", roll_dps, 250.0, "°/s", ACCENT_GREEN);

                            ui.add_space(10.0);
                            ui.separator();
                            ui.add_space(6.0);
                            ui.label(
                                RichText::new(format!(
                                    "Accel:  X {:+.2}g   Y {:+.2}g   Z {:+.2}g",
                                    -st.ay_g, st.az_g, -st.ax_g
                                ))
                                .monospace()
                                .size(11.5)
                                .color(Color32::from_rgb(176, 190, 214)),
                            );
                            let bias_txt = match lang {
                                Language::Es => format!(
                                    "Bias: ({:+.2}, {:+.2}, {:+.2}) °/s · Minus+Plus recalibra",
                                    status.gyro_bias[0], status.gyro_bias[1], status.gyro_bias[2]
                                ),
                                Language::En => format!(
                                    "Bias: ({:+.2}, {:+.2}, {:+.2}) °/s · Minus+Plus recalibrates",
                                    status.gyro_bias[0], status.gyro_bias[1], status.gyro_bias[2]
                                ),
                            };
                            ui.label(RichText::new(bias_txt).size(11.0).color(TEXT_MUTED));
                        },
                    );
                },
            );
        });
    }

    fn ui_paddles(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let btns = status.telemetry.btns;
        let lang = self.cfg.lang;
        card(
            ui,
            match lang {
                Language::Es => "Botones Traseros Independientes (PL / PR)",
                Language::En => "Independent Back Paddles (PL / PR)",
            },
            |ui| {
                ui.label(
                    RichText::new(match lang {
                        Language::Es => "Los botones traseros PL y PR se reciben de forma independiente por radio 2.4 GHz. Presionalos para confirmar cuál corresponde a P1 / P2 y elegí su acción:",
                        Language::En => "The PL and PR back paddles are received independently over 2.4 GHz. Press each paddle to see which indicator lights up and choose its action:",
                    })
                    .color(Color32::from_rgb(185, 194, 212)),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Es => format!("Máscara de botones actual: 0x{:08X}", btns),
                        Language::En => format!("Current raw button bitmask: 0x{:08X}", btns),
                    })
                    .monospace()
                    .size(12.0)
                    .color(ACCENT_CYAN),
                );
                ui.add_space(10.0);

                egui::Grid::new("paddle_grid")
                    .num_columns(3)
                    .spacing([16.0, 12.0])
                    .show(ui, |ui| {
                        paddle_row(
                            ui,
                            match lang {
                                Language::Es => "Trasero PL / P1 (bit 25)",
                                Language::En => "Back Paddle PL / P1 (bit 25)",
                            },
                            (btns & BTN_P1) != 0,
                            "p1",
                            &mut self.cfg.paddle_p1,
                            lang,
                        );
                        paddle_row(
                            ui,
                            match lang {
                                Language::Es => "Trasero PR / P2 (bit 26)",
                                Language::En => "Back Paddle PR / P2 (bit 26)",
                            },
                            (btns & BTN_P2) != 0,
                            "p2",
                            &mut self.cfg.paddle_p2,
                            lang,
                        );
                    });

                ui.add_space(14.0);
                egui::Frame::default()
                    .fill(Color32::from_rgb(26, 31, 44))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(46, 56, 78)))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::same(10))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(match lang {
                                Language::Es => "¿Qué pasa con los botones superiores L4 y R4?",
                                Language::En => "What about the extra L4 and R4 shoulder buttons?",
                            })
                            .strong()
                            .size(12.5)
                            .color(ACCENT_AMBER),
                        );
                        ui.add_space(3.0);
                        ui.label(
                            RichText::new(match lang {
                                Language::Es => "El firmware del dongle 2.4 GHz (paquete de radio 0xA2) solo expone PL y PR como bits crudos independientes; L4 y R4 no se envían como bits separados si están sin asignar. Para usar L4 y R4, asignales cualquier botón desde el propio mando o con 8BitDo Ultimate Software y funcionarán normalmente en todos los juegos.",
                                Language::En => "The 2.4 GHz dongle firmware (0xA2 radio packet) only exposes PL and PR as separate raw bits; unmapped L4 and R4 buttons are not sent separately. To use L4 and R4, map them to any standard button on the controller or via 8BitDo Ultimate Software and they will work in all games.",
                            })
                            .size(12.0)
                            .color(Color32::from_rgb(175, 184, 204)),
                        );
                    });
            },
        );
    }

    fn ui_advanced(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let lang = self.cfg.lang;

        // Window / System Tray behavior card
        card(
            ui,
            match lang {
                Language::Es => "Comportamiento de Ventana y Bandeja del Sistema",
                Language::En => "Window & System Tray Behavior",
            },
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.label(match lang {
                        Language::Es => "Al pulsar X (cerrar ventana):",
                        Language::En => "When clicking X (close window):",
                    });
                    ui.selectable_value(
                        &mut self.cfg.close_action,
                        CloseAction::Ask,
                        match lang {
                            Language::Es => "Preguntar siempre",
                            Language::En => "Always ask",
                        },
                    );
                    ui.selectable_value(
                        &mut self.cfg.close_action,
                        CloseAction::MinimizeToTray,
                        match lang {
                            Language::Es => "Minimizar a la bandeja",
                            Language::En => "Minimize to Tray",
                        },
                    );
                    ui.selectable_value(
                        &mut self.cfg.close_action,
                        CloseAction::Exit,
                        match lang {
                            Language::Es => "Cerrar completamente",
                            Language::En => "Exit completely",
                        },
                    );
                });
            },
        );

        ui.add_space(12.0);

        card(
            ui,
            match lang {
                Language::Es => "Giroscopio Directo para PC (opcional fuera de Steam Input)",
                Language::En => "Direct PC Gyro Aiming (optional outside Steam Input)",
            },
            |ui| {
                ui.label(
                    RichText::new(match lang {
                        Language::Es => "En modo DualShock 4 dejalo en 'Desactivado' para que Steam Input controle el giroscopio nativamente. Usá Gyro → Mouse o Gyro → Stick Derecho solo en juegos fuera de Steam o en modo Xbox 360.",
                        Language::En => "In DualShock 4 mode, leave this Off so Steam Input handles the gyroscope natively. Use Gyro → Mouse or Gyro → Right Stick for non-Steam PC games or Xbox 360 mode.",
                    })
                    .size(12.0)
                    .color(Color32::from_rgb(175, 184, 200)),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label(match lang { Language::Es => "Modo:", Language::En => "Mode:" });
                    ui.selectable_value(
                        &mut self.cfg.gyro_pc_mode,
                        GyroPcMode::Off,
                        match lang {
                            Language::Es => "Desactivado (recomendado en DS4)",
                            Language::En => "Off (recommended for DS4)",
                        },
                    );
                    ui.selectable_value(&mut self.cfg.gyro_pc_mode, GyroPcMode::Mouse, "Gyro → Mouse");
                    ui.selectable_value(
                        &mut self.cfg.gyro_pc_mode,
                        GyroPcMode::RightStick,
                        match lang {
                            Language::Es => "Gyro → Stick Derecho",
                            Language::En => "Gyro → Right Stick",
                        },
                    );
                });

                if self.cfg.gyro_pc_mode != GyroPcMode::Off {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label(match lang { Language::Es => "Activar con:", Language::En => "Activate with:" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::LeftTrigger, match lang { Language::Es => "Mantener L2", Language::En => "Hold L2" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::RightTrigger, match lang { Language::Es => "Mantener R2", Language::En => "Hold R2" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::LeftBumper, match lang { Language::Es => "Mantener L1", Language::En => "Hold L1" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::RightBumper, match lang { Language::Es => "Mantener R1", Language::En => "Hold R1" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::AlwaysOn, match lang { Language::Es => "Siempre activo", Language::En => "Always on" });
                        ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::ToggleCapture, "Toggle (Star)");
                    });
                    ui.add_space(6.0);
                    ui.add(egui::Slider::new(&mut self.cfg.gyro_sens, 0.2..=4.0).text(match lang { Language::Es => "Sensibilidad", Language::En => "Sensitivity" }));
                    ui.add(egui::Slider::new(&mut self.cfg.gyro_roll_mix, 0.0..=1.0).text(match lang { Language::Es => "Mezcla Roll en eje X", Language::En => "Roll mix on X axis" }));
                    ui.add(egui::Slider::new(&mut self.cfg.gyro_deadzone, 0.0..=5.0).text(match lang { Language::Es => "Zona muerta (°/s)", Language::En => "Deadzone (°/s)" }));
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.cfg.invert_x, match lang { Language::Es => "Invertir eje X", Language::En => "Invert X axis" });
                        ui.checkbox(&mut self.cfg.invert_y, match lang { Language::Es => "Invertir eje Y", Language::En => "Invert Y axis" });
                        ui.label(if status.gyro_pc_active {
                            RichText::new(match lang { Language::Es => " · ACTIVO", Language::En => " · ACTIVE" }).color(ACCENT_GREEN).strong()
                        } else {
                            RichText::new(match lang { Language::Es => " · en espera", Language::En => " · standby" }).color(TEXT_MUTED)
                        });
                    });
                }
            },
        );

        ui.add_space(12.0);

        card(
            ui,
            match lang {
                Language::Es => "Servidor DSU / Cemuhook y Guía Rápida",
                Language::En => "DSU / Cemuhook Server & Quick Guide",
            },
            |ui| {
                ui.horizontal(|ui| {
                    ui.checkbox(
                        &mut self.cfg.dsu_enabled,
                        match lang {
                            Language::Es => "Activar servidor UDP DSU",
                            Language::En => "Enable DSU UDP server",
                        },
                    );
                    ui.label(match lang { Language::Es => "Puerto:", Language::En => "Port:" });
                    ui.add(egui::DragValue::new(&mut self.cfg.dsu_port).range(1024..=65535));
                });
                ui.add_space(6.0);
                ui.label(
                    RichText::new(match lang {
                        Language::Es => "• Steam: Parámetros → Mando → Activar Steam Input para mandos de PlayStation.\n• Dolphin: Mandos → Fuentes de entrada alternativas → 127.0.0.1 : 26760.\n• Cemu: Opciones → Entrada → API: DSUController o Motion Source → 127.0.0.1 : 26760.\n• Ryujinx / Yuzu / Eden: Controles → Movimiento → CemuHookUDP → 127.0.0.1 : 26760.\n• Volver al modo XInput de fábrica: cerrá esta app y desenchufá/volvé a enchufar el dongle.",
                        Language::En => "• Steam: Settings → Controller → Enable Steam Input for PlayStation controllers.\n• Dolphin: Controllers → Alternate Input Sources → 127.0.0.1 : 26760.\n• Cemu: Options → Input Settings → API: DSUController or Motion Source → 127.0.0.1 : 26760.\n• Ryujinx / Yuzu / Eden: Controls → Motion → Use CemuHookUDP → 127.0.0.1 : 26760.\n• Restore stock XInput mode: exit this app and unplug/replug the USB dongle.",
                    })
                    .size(12.0)
                    .color(Color32::from_rgb(175, 184, 200)),
                );
            },
        );
    }
}

/// Renders the embedded studio SVG of the 8BitDo Pro 3 (640x430) with live glowing interactive overlays.
fn draw_pro3_controller(
    ui: &mut egui::Ui,
    tex: Option<&egui::TextureHandle>,
    st: &TelemetryState,
    rumble_l: u8,
    rumble_r: u8,
) {
    let avail_w = ui.available_width();
    let w = avail_w.min(480.0);
    let top_pad = 26.0;
    let svg_h = w * (430.0 / 640.0);
    let total_h = svg_h + top_pad;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, total_h), egui::Sense::hover());
    let p = ui.painter();

    let scale = w / 640.0;
    let ox = rect.center().x - 320.0 * scale;
    let oy = rect.top() + top_pad;
    let pt = |x: f32, y: f32| Pos2::new(ox + x * scale, oy + y * scale);
    let sc = |v: f32| v * scale;

    let btns = st.btns;

    // 1) L2 / R2 Analog Trigger Bars right above the shoulders
    let l2_frac = st.l2 as f32 / 255.0;
    let r2_frac = st.r2 as f32 / 255.0;
    let l2_rect = Rect::from_min_max(
        Pos2::new(ox + 92.0 * scale, rect.top() + 2.0),
        Pos2::new(ox + 230.0 * scale, rect.top() + 22.0),
    );
    let r2_rect = Rect::from_min_max(
        Pos2::new(ox + 410.0 * scale, rect.top() + 2.0),
        Pos2::new(ox + 548.0 * scale, rect.top() + 22.0),
    );
    p.rect_filled(l2_rect, CornerRadius::same(6), Color32::from_rgb(16, 20, 32));
    p.rect_filled(r2_rect, CornerRadius::same(6), Color32::from_rgb(16, 20, 32));
    if l2_frac > 0.01 {
        p.rect_filled(
            Rect::from_min_max(
                l2_rect.min,
                Pos2::new(l2_rect.left() + l2_rect.width() * l2_frac, l2_rect.bottom()),
            ),
            CornerRadius::same(5),
            ACCENT_CYAN,
        );
    }
    if r2_frac > 0.01 {
        p.rect_filled(
            Rect::from_min_max(
                Pos2::new(r2_rect.right() - r2_rect.width() * r2_frac, r2_rect.top()),
                r2_rect.max,
            ),
            CornerRadius::same(5),
            ACCENT_CORAL,
        );
    }
    p.rect_stroke(l2_rect, CornerRadius::same(6), Stroke::new(1.2, Color32::from_rgb(56, 66, 94)), egui::StrokeKind::Outside);
    p.rect_stroke(r2_rect, CornerRadius::same(6), Stroke::new(1.2, Color32::from_rgb(56, 66, 94)), egui::StrokeKind::Outside);
    p.text(l2_rect.center(), Align2::CENTER_CENTER, format!("L2  {:3}", st.l2), FontId::proportional(11.5), Color32::WHITE);
    p.text(r2_rect.center(), Align2::CENTER_CENTER, format!("R2  {:3}", st.r2), FontId::proportional(11.5), Color32::WHITE);

    // 2) Embedded 8BitDo Pro 3 SVG texture
    let img_rect = Rect::from_min_size(Pos2::new(ox, oy), Vec2::new(640.0 * scale, 430.0 * scale));
    if let Some(t) = tex {
        p.image(
            t.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // 3) Rumble glow halos on left / right grips
    if rumble_l > 0 {
        let a = ((rumble_l as f32 / 255.0) * 115.0) as u8;
        p.circle_filled(
            pt(112.0, 326.0),
            sc(64.0),
            Color32::from_rgba_unmultiplied(ACCENT_AMBER.r(), ACCENT_AMBER.g(), ACCENT_AMBER.b(), a),
        );
    }
    if rumble_r > 0 {
        let a = ((rumble_r as f32 / 255.0) * 115.0) as u8;
        p.circle_filled(
            pt(528.0, 326.0),
            sc(64.0),
            Color32::from_rgba_unmultiplied(ACCENT_AMBER.r(), ACCENT_AMBER.g(), ACCENT_AMBER.b(), a),
        );
    }

    // 4) L1 / R1 Bumper Glow Overlays
    if (btns & BTN_L1) != 0 {
        let r = Rect::from_min_max(pt(82.0, 56.0), pt(208.0, 78.0));
        p.rect_filled(r, CornerRadius::same(7), Color32::from_rgba_unmultiplied(56, 189, 248, 170));
    }
    if (btns & BTN_R1) != 0 {
        let r = Rect::from_min_max(pt(432.0, 56.0), pt(558.0, 78.0));
        p.rect_filled(r, CornerRadius::same(7), Color32::from_rgba_unmultiplied(56, 189, 248, 170));
    }

    // 5) Cross D-Pad Directional Highlights
    let d_glow = Color32::from_rgba_unmultiplied(56, 189, 248, 190);
    if (btns & BTN_DUP) != 0 {
        p.rect_filled(Rect::from_min_max(pt(137.0, 118.0), pt(167.0, 152.0)), CornerRadius::same(4), d_glow);
    }
    if (btns & BTN_DDOWN) != 0 {
        p.rect_filled(Rect::from_min_max(pt(137.0, 176.0), pt(167.0, 210.0)), CornerRadius::same(4), d_glow);
    }
    if (btns & BTN_DLEFT) != 0 {
        p.rect_filled(Rect::from_min_max(pt(106.0, 149.0), pt(140.0, 179.0)), CornerRadius::same(4), d_glow);
    }
    if (btns & BTN_DRIGHT) != 0 {
        p.rect_filled(Rect::from_min_max(pt(164.0, 149.0), pt(198.0, 179.0)), CornerRadius::same(4), d_glow);
    }

    // 6) Face Button Luminous Halos
    let glow_circle = |pos: Pos2, radius: f32, col: Color32| {
        p.circle_filled(
            pos,
            radius * 1.38,
            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 85),
        );
        p.circle_filled(
            pos,
            radius,
            Color32::from_rgba_unmultiplied(col.r(), col.g(), col.b(), 155),
        );
        p.circle_stroke(pos, radius, Stroke::new(2.2, Color32::WHITE));
    };
    if (btns & BTN_NORTH) != 0 { glow_circle(pt(488.0, 124.0), sc(20.0), ACCENT_CYAN); }
    if (btns & BTN_WEST) != 0  { glow_circle(pt(440.0, 164.0), sc(20.0), ACCENT_CYAN); }
    if (btns & BTN_EAST) != 0  { glow_circle(pt(536.0, 164.0), sc(20.0), ACCENT_GREEN); }
    if (btns & BTN_SOUTH) != 0 { glow_circle(pt(488.0, 204.0), sc(20.0), ACCENT_CORAL); }

    // 7) -select / +start / Star / Home Overlays
    if (btns & BTN_MINUS) != 0 {
        p.rect_filled(
            Rect::from_min_max(pt(266.0, 149.0), pt(314.0, 167.0)),
            CornerRadius::same(7),
            Color32::from_rgba_unmultiplied(56, 189, 248, 200),
        );
    }
    if (btns & BTN_PLUS) != 0 {
        p.rect_filled(
            Rect::from_min_max(pt(326.0, 149.0), pt(374.0, 167.0)),
            CornerRadius::same(7),
            Color32::from_rgba_unmultiplied(56, 189, 248, 200),
        );
    }
    if (btns & BTN_CAPTURE) != 0 {
        glow_circle(pt(152.0, 242.0), sc(13.0), ACCENT_AMBER);
    }
    if (btns & BTN_HOME) != 0 {
        glow_circle(pt(488.0, 242.0), sc(14.0), ACCENT_VIOLET);
    }

    // 8) Live Animated Analog Sticks
    let draw_stick_overlay = |cx: f32, cy: f32, x: u8, y: u8, clicked: bool| {
        let center = pt(cx, cy);
        let nx = ((x as f32) - 127.5) / 127.5;
        let ny = ((y as f32) - 127.5) / 127.5;
        let moved = nx.abs() > 0.04 || ny.abs() > 0.04;
        if moved || clicked {
            p.circle_filled(center, sc(36.0), Color32::from_rgb(33, 35, 84));
            let travel = sc(16.0);
            let knob = Pos2::new(center.x + nx * travel, center.y + ny * travel);
            p.circle_filled(
                knob,
                sc(27.0),
                if clicked {
                    Color32::from_rgb(48, 198, 146)
                } else {
                    Color32::from_rgb(218, 224, 236)
                },
            );
            p.circle_stroke(
                knob,
                sc(27.0),
                Stroke::new(2.0, if clicked { Color32::WHITE } else { ACCENT_CYAN }),
            );
            p.circle_filled(
                knob,
                sc(18.0),
                if clicked {
                    Color32::from_rgb(32, 160, 118)
                } else {
                    Color32::from_rgb(196, 204, 218)
                },
            );
        }
    };
    draw_stick_overlay(234.0, 240.0, st.lx, st.ly, (btns & BTN_L3) != 0);
    draw_stick_overlay(406.0, 240.0, st.rx, st.ry, (btns & BTN_R3) != 0);

    // 9) PL / PR Rear Paddle Badges
    let p1_on = (btns & BTN_P1) != 0;
    let p2_on = (btns & BTN_P2) != 0;
    let pl_rect = Rect::from_min_max(pt(242.0, 326.0), pt(310.0, 356.0));
    let pr_rect = Rect::from_min_max(pt(330.0, 326.0), pt(398.0, 356.0));
    p.rect_filled(pl_rect, CornerRadius::same(8), if p1_on { ACCENT_GREEN } else { Color32::from_rgb(22, 28, 44) });
    p.rect_stroke(pl_rect, CornerRadius::same(8), Stroke::new(1.3, if p1_on { ACCENT_GREEN } else { Color32::from_rgb(76, 94, 148) }), egui::StrokeKind::Outside);
    p.text(pl_rect.center(), Align2::CENTER_CENTER, "PL", FontId::proportional(sc(14.0)), if p1_on { Color32::BLACK } else { Color32::from_rgb(185, 198, 232) });

    p.rect_filled(pr_rect, CornerRadius::same(8), if p2_on { ACCENT_GREEN } else { Color32::from_rgb(22, 28, 44) });
    p.rect_stroke(pr_rect, CornerRadius::same(8), Stroke::new(1.3, if p2_on { ACCENT_GREEN } else { Color32::from_rgb(76, 94, 148) }), egui::StrokeKind::Outside);
    p.text(pr_rect.center(), Align2::CENTER_CENTER, "PR", FontId::proportional(sc(14.0)), if p2_on { Color32::BLACK } else { Color32::from_rgb(185, 198, 232) });
}

/// Live artificial horizon / tilt indicator driven by 3-axis accelerometer + yaw arc.
fn draw_horizon_gauge(ui: &mut egui::Ui, ax: f32, ay: f32, az: f32, yaw_dps: f32) {
    ui.vertical_centered(|ui| {
        let size = 122.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let p = ui.painter();
        let center = rect.center();
        let r = size * 0.46;

        p.circle_filled(center, r, Color32::from_rgb(13, 17, 26));
        p.circle_stroke(center, r, Stroke::new(1.6, Color32::from_rgb(50, 60, 84)));

        let roll_rad = ax.atan2(ay.max(0.05)).clamp(-1.1, 1.1);
        let pitch_rad = az.atan2((ax * ax + ay * ay).sqrt().max(0.05)).clamp(-1.0, 1.0);
        let pitch_px = pitch_rad * (r * 0.65);

        let cos_r = roll_rad.cos();
        let sin_r = roll_rad.sin();
        let half_len = r * 0.78;
        let mid = Pos2::new(center.x - sin_r * pitch_px, center.y + cos_r * pitch_px);
        let p1 = Pos2::new(mid.x - cos_r * half_len, mid.y - sin_r * half_len);
        let p2 = Pos2::new(mid.x + cos_r * half_len, mid.y + sin_r * half_len);

        p.line_segment([p1, p2], Stroke::new(2.4, ACCENT_CYAN));
        p.circle_stroke(center, 5.0, Stroke::new(1.5, ACCENT_AMBER));
        p.line_segment(
            [Pos2::new(center.x - 14.0, center.y), Pos2::new(center.x - 6.0, center.y)],
            Stroke::new(1.5, ACCENT_AMBER),
        );
        p.line_segment(
            [Pos2::new(center.x + 6.0, center.y), Pos2::new(center.x + 14.0, center.y)],
            Stroke::new(1.5, ACCENT_AMBER),
        );

        let yaw_angle = -std::f32::consts::FRAC_PI_2 - (yaw_dps / 240.0).clamp(-1.0, 1.0) * 1.1;
        let yaw_dot = Pos2::new(
            center.x + yaw_angle.cos() * (r + 1.0),
            center.y + yaw_angle.sin() * (r + 1.0),
        );
        p.circle_filled(yaw_dot, 4.2, ACCENT_VIOLET);
    });
}

fn tab_button(ui: &mut egui::Ui, current: &mut Tab, target: Tab, label: &str) {
    let selected = *current == target;
    let fill = if selected {
        Color32::from_rgb(32, 54, 96)
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if selected {
        Stroke::new(1.0, Color32::from_rgb(76, 136, 232))
    } else {
        Stroke::NONE
    };
    let text_col = if selected { TEXT_PRIMARY } else { TEXT_MUTED };
    if ui
        .add(
            egui::Button::new(RichText::new(label).size(13.0).color(text_col))
                .fill(fill)
                .stroke(stroke)
                .corner_radius(CornerRadius::same(7))
                .min_size(Vec2::new(0.0, 28.0)),
        )
        .clicked()
    {
        *current = target;
    }
}

fn paddle_row(
    ui: &mut egui::Ui,
    label: &str,
    pressed: bool,
    id: &str,
    action: &mut PaddleAction,
    lang: Language,
) {
    let dot_color = if pressed {
        ACCENT_GREEN
    } else {
        Color32::from_rgb(58, 66, 84)
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(11.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, dot_color);
        ui.label(RichText::new(label).strong().color(TEXT_PRIMARY));
    });
    egui::ComboBox::from_id_salt(id)
        .selected_text(action.label(lang))
        .width(250.0)
        .show_ui(ui, |ui| {
            for &opt in PaddleAction::ALL {
                ui.selectable_value(action, opt, opt.label(lang));
            }
        });
    ui.label(if pressed {
        RichText::new(match lang { Language::Es => "PULSADO", Language::En => "PRESSED" })
            .color(ACCENT_GREEN)
            .strong()
    } else {
        RichText::new("")
    });
    ui.end_row();
}

fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, BORDER_CARD))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(14))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(RichText::new(title).size(13.5).strong().color(TEXT_PRIMARY));
            ui.add_space(8.0);
            add_contents(ui);
        });
}

fn status_pill(ui: &mut egui::Ui, text: &str, bg: Color32, fg: Color32) {
    egui::Frame::default()
        .fill(bg)
        .stroke(Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(fg.r(), fg.g(), fg.b(), 70),
        ))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.5).strong().color(fg));
        });
}

fn draw_signed_bar(
    ui: &mut egui::Ui,
    label: &str,
    value: f32,
    max_abs: f32,
    unit: &str,
    color: Color32,
) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{label} {:+6.1} {unit}", value))
                .monospace()
                .size(11.5)
                .color(TEXT_PRIMARY),
        );
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 12.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(13, 16, 24));
        let mid_x = rect.center().x;
        p.line_segment(
            [Pos2::new(mid_x, rect.top()), Pos2::new(mid_x, rect.bottom())],
            Stroke::new(1.0, Color32::from_rgb(64, 74, 96)),
        );
        let norm = (value / max_abs).clamp(-1.0, 1.0);
        let half = rect.width() * 0.5;
        let fill_rect = if norm >= 0.0 {
            Rect::from_min_max(
                Pos2::new(mid_x, rect.top()),
                Pos2::new(mid_x + norm * half, rect.bottom()),
            )
        } else {
            Rect::from_min_max(
                Pos2::new(mid_x + norm * half, rect.top()),
                Pos2::new(mid_x, rect.bottom()),
            )
        };
        if fill_rect.width() > 0.5 {
            p.rect_filled(fill_rect, CornerRadius::same(3), color);
        }
    });
}
