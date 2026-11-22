use crate::bridge::{BridgeControl, BridgeStatus};
use crate::config::*;
use crate::protocol::*;
use eframe::egui::{self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, RichText, Stroke, Vec2};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

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
}

impl Pro3App {
    pub fn new(ctrl: Arc<BridgeControl>, cfg: BridgeConfig) -> Self {
        Self {
            ctrl,
            cfg,
            tab: Tab::Dashboard,
        }
    }

    fn sync_config(&self) {
        if let Ok(mut c) = self.ctrl.config.lock() {
            *c = self.cfg.clone();
        }
        self.cfg.save();
    }
}

impl eframe::App for Pro3App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.request_repaint_after(Duration::from_millis(16));

        let status = self
            .ctrl
            .status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_default();

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
                                RichText::new(&status.state_text)
                                    .size(12.5)
                                    .color(Color32::from_rgb(176, 186, 206)),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let is_running = self.ctrl.running.load(Ordering::Relaxed);
                        let btn_text = if is_running { "Detener" } else { "Iniciar" };
                        let btn_fill = if is_running {
                            Color32::from_rgb(185, 52, 68)
                        } else {
                            Color32::from_rgb(28, 148, 96)
                        };
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(btn_text).strong().size(13.0).color(Color32::WHITE),
                                )
                                .fill(btn_fill)
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(88.0, 32.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.running.store(!is_running, Ordering::Relaxed);
                        }

                        if ui
                            .add_enabled(
                                status.connected,
                                egui::Button::new(RichText::new("Vibrar").size(12.5))
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
                                egui::Button::new(RichText::new("Calibrar Gyro").size(12.5))
                                    .corner_radius(CornerRadius::same(8))
                                    .min_size(Vec2::new(104.0, 32.0)),
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
                    status_pill(
                        ui,
                        &format!("Batería {}%", status.battery),
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
                    status_pill(
                        ui,
                        &status.vigem_status,
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
                            format!("DSU: {err}")
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
                    tab_button(ui, &mut self.tab, Tab::Dashboard, "Panel en Vivo");
                    tab_button(ui, &mut self.tab, Tab::Paddles, "Botones Traseros (PL / PR)");
                    tab_button(ui, &mut self.tab, Tab::Advanced, "Gyro PC & Emuladores");
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
    }
}

impl Pro3App {
    fn ui_dashboard(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let st = &status.telemetry;

        // Top compact output bar
        card(ui, "Perfil de Salida", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Mando:").color(TEXT_MUTED));
                ui.selectable_value(
                    &mut self.cfg.pad_mode,
                    PadMode::Ds4NativeGyro,
                    "DualShock 4 (Gyro Nativo Steam)",
                );
                ui.selectable_value(&mut self.cfg.pad_mode, PadMode::Xbox360, "Xbox 360");
                ui.selectable_value(&mut self.cfg.pad_mode, PadMode::None, "Solo DSU");

                ui.separator();
                ui.label(RichText::new("Botones:").color(TEXT_MUTED));
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::XboxPs, "Xbox / PS");
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::Nintendo, "Nintendo");
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.checkbox(&mut self.cfg.dsu_enabled, "DSU Emuladores (UDP)");
                ui.separator();
                ui.checkbox(&mut self.cfg.rumble_enabled, "Vibración");
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

        // Main visual showcase: Left = Interactive 8BitDo Pro 3 Vector Graphic, Right = Live Gyro Horizon + Telemetry
        ui.horizontal_top(|ui| {
            let total_w = ui.available_width();
            let left_w = (total_w * 0.58).max(420.0);
            let right_w = (total_w - left_w - 12.0).max(240.0);

            ui.allocate_ui_with_layout(
                Vec2::new(left_w, 340.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    card(ui, "8BitDo Pro 3 · Vista Interactiva en Vivo", |ui| {
                        draw_pro3_controller(
                            ui,
                            st,
                            self.cfg.layout,
                            status.rumble_l,
                            status.rumble_r,
                        );
                    });
                },
            );

            ui.add_space(12.0);

            ui.allocate_ui_with_layout(
                Vec2::new(right_w, 340.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    card(ui, "Giroscopio y Orientación (200 Hz)", |ui| {
                        let gx = st.gx_dps - status.gyro_bias[0];
                        let gy = st.gy_dps - status.gyro_bias[1];
                        let gz = st.gz_dps - status.gyro_bias[2];
                        let pitch_dps = -gy;
                        let yaw_dps = gz;
                        let roll_dps = -gx;

                        // Live artificial horizon driven by accelerometer gravity vector
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
                        ui.label(
                            RichText::new(format!(
                                "Bias: ({:+.2}, {:+.2}, {:+.2}) °/s · Minus+Plus recalibra",
                                status.gyro_bias[0], status.gyro_bias[1], status.gyro_bias[2]
                            ))
                            .size(11.0)
                            .color(TEXT_MUTED),
                        );
                    });
                },
            );
        });
    }

    fn ui_paddles(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let btns = status.telemetry.btns;
        card(ui, "Botones Traseros Independientes (PL / PR)", |ui| {
            ui.label(
                RichText::new(
                    "Los botones traseros PL y PR se reciben de forma independiente por radio 2.4 GHz. Presionalos para confirmar cuál corresponde a P1 / P2 y elegí su acción:",
                )
                .color(Color32::from_rgb(185, 194, 212)),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Máscara de botones actual: 0x{:08X}", btns))
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
                        "Trasero PL / P1 (bit 25)",
                        (btns & BTN_P1) != 0,
                        "p1",
                        &mut self.cfg.paddle_p1,
                    );
                    paddle_row(
                        ui,
                        "Trasero PR / P2 (bit 26)",
                        (btns & BTN_P2) != 0,
                        "p2",
                        &mut self.cfg.paddle_p2,
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
                        RichText::new("¿Qué pasa con los botones superiores L4 y R4?")
                            .strong()
                            .size(12.5)
                            .color(ACCENT_AMBER),
                    );
                    ui.add_space(3.0);
                    ui.label(
                        RichText::new(
                            "El firmware del dongle 2.4 GHz (paquete de radio 0xA2) solo expone PL y PR como bits crudos independientes; L4 y R4 no se envían como bits separados si están sin asignar. Para usar L4 y R4, asignales cualquier botón desde el propio mando o con 8BitDo Ultimate Software (por ejemplo L3/R3 o A/B) y funcionarán normalmente en todos los juegos.",
                        )
                        .size(12.0)
                        .color(Color32::from_rgb(175, 184, 204)),
                    );
                });
        });
    }

    fn ui_advanced(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        card(
            ui,
            "Giroscopio Directo para PC (opcional fuera de Steam Input)",
            |ui| {
                ui.label(
                    RichText::new(
                        "En modo DualShock 4 dejalo en 'Desactivado' para que Steam Input controle el giroscopio nativamente. Usá Gyro → Mouse o Gyro → Stick Derecho solo en juegos fuera de Steam o en modo Xbox 360.",
                    )
                    .size(12.0)
                    .color(Color32::from_rgb(175, 184, 200)),
                );
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    ui.label("Modo:");
                    ui.selectable_value(
                        &mut self.cfg.gyro_pc_mode,
                        GyroPcMode::Off,
                        "Desactivado (recomendado en DS4)",
                    );
                    ui.selectable_value(&mut self.cfg.gyro_pc_mode, GyroPcMode::Mouse, "Gyro → Mouse");
                    ui.selectable_value(
                        &mut self.cfg.gyro_pc_mode,
                        GyroPcMode::RightStick,
                        "Gyro → Stick Derecho",
                    );
                });

                if self.cfg.gyro_pc_mode != GyroPcMode::Off {
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Activar con:");
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::LeftTrigger,
                            "Mantener L2",
                        );
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::RightTrigger,
                            "Mantener R2",
                        );
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::LeftBumper,
                            "Mantener L1",
                        );
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::RightBumper,
                            "Mantener R1",
                        );
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::AlwaysOn,
                            "Siempre activo",
                        );
                        ui.selectable_value(
                            &mut self.cfg.gyro_trigger,
                            GyroTrigger::ToggleCapture,
                            "Toggle (Star)",
                        );
                    });
                    ui.add_space(6.0);
                    ui.add(
                        egui::Slider::new(&mut self.cfg.gyro_sens, 0.2..=4.0).text("Sensibilidad"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.cfg.gyro_roll_mix, 0.0..=1.0)
                            .text("Mezcla Roll en eje X"),
                    );
                    ui.add(
                        egui::Slider::new(&mut self.cfg.gyro_deadzone, 0.0..=5.0)
                            .text("Zona muerta (°/s)"),
                    );
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.cfg.invert_x, "Invertir eje X");
                        ui.checkbox(&mut self.cfg.invert_y, "Invertir eje Y");
                        ui.label(if status.gyro_pc_active {
                            RichText::new(" · ACTIVO").color(ACCENT_GREEN).strong()
                        } else {
                            RichText::new(" · en espera").color(TEXT_MUTED)
                        });
                    });
                }
            },
        );

        ui.add_space(12.0);

        card(ui, "Servidor DSU / Cemuhook y Guía Rápida", |ui| {
            ui.horizontal(|ui| {
                ui.checkbox(&mut self.cfg.dsu_enabled, "Activar servidor UDP DSU");
                ui.label("Puerto:");
                ui.add(egui::DragValue::new(&mut self.cfg.dsu_port).range(1024..=65535));
            });
            ui.add_space(6.0);
            ui.label(
                RichText::new(
                    "• Steam: Parámetros → Mando → Activar Steam Input para mandos de PlayStation.\n\
                     • Dolphin: Mandos → Fuentes de entrada alternativas → 127.0.0.1 : 26760.\n\
                     • Cemu: Opciones → Entrada → API: DSUController o Motion Source → 127.0.0.1 : 26760.\n\
                     • Ryujinx / Yuzu / Eden: Controles → Movimiento → CemuHookUDP → 127.0.0.1 : 26760.\n\
                     • Volver al modo XInput de fábrica: cerrá esta app y desenchufá/volvé a enchufar el dongle.",
                )
                .size(12.0)
                .color(Color32::from_rgb(175, 184, 200)),
            );
        });
    }
}

/// Interactive vector drawing of the 8BitDo Pro 3 controller with real-time button/stick/trigger/rumble feedback.
fn draw_pro3_controller(
    ui: &mut egui::Ui,
    st: &TelemetryState,
    layout: ButtonLayout,
    rumble_l: u8,
    rumble_r: u8,
) {
    let avail_w = ui.available_width();
    let w = avail_w.min(480.0);
    let h = w * 0.68;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, h), egui::Sense::hover());
    let p = ui.painter();

    // Helper mapping normalized 640x420 SVG coordinates onto the centered rect
    let scale = (w / 640.0).min(h / 420.0);
    let ox = rect.center().x - 320.0 * scale;
    let oy = rect.center().y - 210.0 * scale;
    let pt = |x: f32, y: f32| Pos2::new(ox + x * scale, oy + y * scale);
    let sc = |v: f32| v * scale;

    let btns = st.btns;

    // 1) L2 / R2 Analog Triggers (top shoulders with smooth analog fill!)
    let l2_frac = st.l2 as f32 / 255.0;
    let r2_frac = st.r2 as f32 / 255.0;
    let l2_rect = Rect::from_min_max(pt(115.0, 26.0), pt(220.0, 52.0));
    let r2_rect = Rect::from_min_max(pt(420.0, 26.0), pt(525.0, 52.0));
    p.rect_filled(l2_rect, CornerRadius::same(7), Color32::from_rgb(24, 29, 42));
    p.rect_filled(r2_rect, CornerRadius::same(7), Color32::from_rgb(24, 29, 42));
    if l2_frac > 0.01 {
        let fill = Rect::from_min_max(
            Pos2::new(l2_rect.left(), l2_rect.top()),
            Pos2::new(l2_rect.left() + l2_rect.width() * l2_frac, l2_rect.bottom()),
        );
        p.rect_filled(fill, CornerRadius::same(6), ACCENT_CYAN);
    }
    if r2_frac > 0.01 {
        let fill = Rect::from_min_max(
            Pos2::new(r2_rect.right() - r2_rect.width() * r2_frac, r2_rect.top()),
            Pos2::new(r2_rect.right(), r2_rect.bottom()),
        );
        p.rect_filled(fill, CornerRadius::same(6), ACCENT_CORAL);
    }
    p.rect_stroke(l2_rect, CornerRadius::same(7), Stroke::new(1.3, Color32::from_rgb(66, 78, 105)), egui::StrokeKind::Outside);
    p.rect_stroke(r2_rect, CornerRadius::same(7), Stroke::new(1.3, Color32::from_rgb(66, 78, 105)), egui::StrokeKind::Outside);
    p.text(l2_rect.center(), Align2::CENTER_CENTER, format!("L2  {}", st.l2), FontId::proportional(sc(14.0)), Color32::WHITE);
    p.text(r2_rect.center(), Align2::CENTER_CENTER, format!("R2  {}", st.r2), FontId::proportional(sc(14.0)), Color32::WHITE);

    // 2) L1 / R1 Bumpers
    let l1_on = (btns & BTN_L1) != 0;
    let r1_on = (btns & BTN_R1) != 0;
    let l1_rect = Rect::from_min_max(pt(104.0, 62.0), pt(228.0, 86.0));
    let r1_rect = Rect::from_min_max(pt(412.0, 62.0), pt(536.0, 86.0));
    p.rect_filled(
        l1_rect,
        CornerRadius::same(7),
        if l1_on { ACCENT_CYAN } else { Color32::from_rgb(36, 42, 58) },
    );
    p.rect_filled(
        r1_rect,
        CornerRadius::same(7),
        if r1_on { ACCENT_CYAN } else { Color32::from_rgb(36, 42, 58) },
    );
    p.text(l1_rect.center(), Align2::CENTER_CENTER, "L1", FontId::proportional(sc(14.0)), if l1_on { Color32::BLACK } else { TEXT_PRIMARY });
    p.text(r1_rect.center(), Align2::CENTER_CENTER, "R1", FontId::proportional(sc(14.0)), if r1_on { Color32::BLACK } else { TEXT_PRIMARY });

    // 3) PL / PR Back Paddle Indicators (glow emerald when pressed!)
    let p1_on = (btns & BTN_P1) != 0;
    let p2_on = (btns & BTN_P2) != 0;
    let pl_rect = Rect::from_min_max(pt(146.0, 344.0), pt(218.0, 376.0));
    let pr_rect = Rect::from_min_max(pt(422.0, 344.0), pt(494.0, 376.0));
    p.rect_filled(
        pl_rect,
        CornerRadius::same(8),
        if p1_on { ACCENT_GREEN } else { Color32::from_rgb(22, 32, 46) },
    );
    p.rect_stroke(
        pl_rect,
        CornerRadius::same(8),
        Stroke::new(1.4, if p1_on { ACCENT_GREEN } else { Color32::from_rgb(56, 128, 178) }),
        egui::StrokeKind::Outside,
    );
    p.text(
        pl_rect.center(),
        Align2::CENTER_CENTER,
        "PL / P1",
        FontId::proportional(sc(13.0)),
        if p1_on { Color32::BLACK } else { ACCENT_CYAN },
    );

    p.rect_filled(
        pr_rect,
        CornerRadius::same(8),
        if p2_on { ACCENT_GREEN } else { Color32::from_rgb(22, 32, 46) },
    );
    p.rect_stroke(
        pr_rect,
        CornerRadius::same(8),
        Stroke::new(1.4, if p2_on { ACCENT_GREEN } else { Color32::from_rgb(56, 128, 178) }),
        egui::StrokeKind::Outside,
    );
    p.text(
        pr_rect.center(),
        Align2::CENTER_CENTER,
        "PR / P2",
        FontId::proportional(sc(13.0)),
        if p2_on { Color32::BLACK } else { ACCENT_CYAN },
    );

    // 4) Bottom Ergonomic Grips (with subtle rumble glow!)
    let left_grip_pts = vec![
        pt(96.0, 198.0),
        pt(76.0, 278.0),
        pt(78.0, 336.0),
        pt(108.0, 368.0),
        pt(148.0, 358.0),
        pt(205.0, 272.0),
    ];
    let right_grip_pts = vec![
        pt(544.0, 198.0),
        pt(564.0, 278.0),
        pt(562.0, 336.0),
        pt(532.0, 368.0),
        pt(492.0, 358.0),
        pt(435.0, 272.0),
    ];
    let grip_stroke_l = if rumble_l > 0 {
        Stroke::new(2.5, ACCENT_AMBER)
    } else {
        Stroke::new(1.8, Color32::from_rgb(58, 67, 92))
    };
    let grip_stroke_r = if rumble_r > 0 {
        Stroke::new(2.5, ACCENT_AMBER)
    } else {
        Stroke::new(1.8, Color32::from_rgb(58, 67, 92))
    };
    p.add(egui::Shape::convex_polygon(
        left_grip_pts,
        Color32::from_rgb(24, 28, 40),
        grip_stroke_l,
    ));
    p.add(egui::Shape::convex_polygon(
        right_grip_pts,
        Color32::from_rgb(24, 28, 40),
        grip_stroke_r,
    ));

    // 5) Main Pro 3 Body (SNES twin rounded pads + central bridge)
    let body_fill = Color32::from_rgb(33, 38, 52);
    let body_stroke = Stroke::new(2.0, Color32::from_rgb(70, 82, 110));
    let bridge_rect = Rect::from_min_max(pt(166.0, 96.0), pt(474.0, 292.0));
    p.rect_filled(bridge_rect, CornerRadius::same(22), body_fill);
    p.circle_filled(pt(170.0, 190.0), sc(94.0), body_fill);
    p.circle_filled(pt(470.0, 190.0), sc(94.0), body_fill);
    p.circle_stroke(pt(170.0, 190.0), sc(94.0), body_stroke);
    p.circle_stroke(pt(470.0, 190.0), sc(94.0), body_stroke);
    // Clean over-fill center bridge so inner circle arcs stay hidden
    p.rect_filled(
        Rect::from_min_max(pt(172.0, 98.0), pt(468.0, 290.0)),
        CornerRadius::same(16),
        body_fill,
    );
    p.line_segment([pt(172.0, 96.0), pt(468.0, 96.0)], body_stroke);
    p.line_segment([pt(208.0, 292.0), pt(432.0, 292.0)], body_stroke);

    // Inner SNES circular face wells
    p.circle_filled(pt(170.0, 190.0), sc(68.0), Color32::from_rgb(24, 28, 39));
    p.circle_stroke(pt(170.0, 190.0), sc(68.0), Stroke::new(1.2, Color32::from_rgb(48, 56, 78)));
    p.circle_filled(pt(470.0, 190.0), sc(68.0), Color32::from_rgb(24, 28, 39));
    p.circle_stroke(pt(470.0, 190.0), sc(68.0), Stroke::new(1.2, Color32::from_rgb(48, 56, 78)));

    // Top accent LED bar + title
    p.rect_filled(
        Rect::from_min_max(pt(286.0, 110.0), pt(354.0, 115.0)),
        CornerRadius::same(3),
        ACCENT_CYAN,
    );
    p.text(
        pt(320.0, 134.0),
        Align2::CENTER_CENTER,
        "8BitDo PRO 3",
        FontId::proportional(sc(14.5)),
        Color32::from_rgb(156, 168, 194),
    );

    // 6) Classic Cross D-Pad (Left) with individual directional highlight
    let d_bg = Color32::from_rgb(18, 22, 31);
    let d_lit = ACCENT_CYAN;
    let d_stroke = Stroke::new(1.2, Color32::from_rgb(66, 76, 102));
    // Center square
    p.rect_filled(Rect::from_min_max(pt(154.0, 174.0), pt(186.0, 206.0)), CornerRadius::same(2), d_bg);
    // Up, Down, Left, Right arms
    let d_arms = [
        (Rect::from_min_max(pt(154.0, 138.0), pt(186.0, 176.0)), (btns & BTN_DUP) != 0),
        (Rect::from_min_max(pt(154.0, 204.0), pt(186.0, 242.0)), (btns & BTN_DDOWN) != 0),
        (Rect::from_min_max(pt(118.0, 174.0), pt(156.0, 206.0)), (btns & BTN_DLEFT) != 0),
        (Rect::from_min_max(pt(184.0, 174.0), pt(222.0, 206.0)), (btns & BTN_DRIGHT) != 0),
    ];
    for (r, on) in d_arms {
        p.rect_filled(r, CornerRadius::same(5), if on { d_lit } else { d_bg });
        p.rect_stroke(r, CornerRadius::same(5), d_stroke, egui::StrokeKind::Outside);
    }
    p.circle_filled(pt(170.0, 190.0), sc(9.0), Color32::from_rgb(28, 33, 46));

    // 7) ABXY Face Buttons (Right)
    let (lbl_s, lbl_e, lbl_w, lbl_n) = if layout == ButtonLayout::Nintendo {
        ("B", "A", "Y", "X")
    } else {
        ("A", "B", "X", "Y")
    };
    let face_btns = [
        (pt(470.0, 230.0), lbl_s, (btns & BTN_SOUTH) != 0, ACCENT_GREEN),
        (pt(510.0, 190.0), lbl_e, (btns & BTN_EAST) != 0, ACCENT_CORAL),
        (pt(430.0, 190.0), lbl_w, (btns & BTN_WEST) != 0, ACCENT_CYAN),
        (pt(470.0, 150.0), lbl_n, (btns & BTN_NORTH) != 0, ACCENT_AMBER),
    ];
    for (pos, lbl, on, accent) in face_btns {
        if on {
            p.circle_filled(
                pos,
                sc(22.0),
                Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 65),
            );
        }
        p.circle_filled(pos, sc(16.5), if on { accent } else { Color32::from_rgb(28, 33, 46) });
        p.circle_stroke(pos, sc(16.5), Stroke::new(1.8, accent));
        p.text(
            pos,
            Align2::CENTER_CENTER,
            lbl,
            FontId::proportional(sc(15.0)),
            if on { Color32::BLACK } else { accent },
        );
    }

    // 8) Center Buttons (Minus, Plus, Star/Capture, Home)
    let min_on = (btns & BTN_MINUS) != 0;
    let pls_on = (btns & BTN_PLUS) != 0;
    let cap_on = (btns & BTN_CAPTURE) != 0;
    let hom_on = (btns & BTN_HOME) != 0;

    let min_r = Rect::from_center_size(pt(288.0, 174.0), Vec2::new(sc(34.0), sc(14.0)));
    let pls_r = Rect::from_center_size(pt(352.0, 174.0), Vec2::new(sc(34.0), sc(14.0)));
    p.rect_filled(min_r, CornerRadius::same(7), if min_on { ACCENT_CYAN } else { Color32::from_rgb(22, 26, 37) });
    p.rect_stroke(min_r, CornerRadius::same(7), Stroke::new(1.2, Color32::from_rgb(74, 86, 114)), egui::StrokeKind::Outside);
    p.text(min_r.center(), Align2::CENTER_CENTER, "−", FontId::proportional(sc(14.0)), if min_on { Color32::BLACK } else { TEXT_PRIMARY });

    p.rect_filled(pls_r, CornerRadius::same(7), if pls_on { ACCENT_CYAN } else { Color32::from_rgb(22, 26, 37) });
    p.rect_stroke(pls_r, CornerRadius::same(7), Stroke::new(1.2, Color32::from_rgb(74, 86, 114)), egui::StrokeKind::Outside);
    p.text(pls_r.center(), Align2::CENTER_CENTER, "+", FontId::proportional(sc(14.0)), if pls_on { Color32::BLACK } else { TEXT_PRIMARY });

    p.circle_filled(pt(288.0, 208.0), sc(11.0), if cap_on { ACCENT_AMBER } else { Color32::from_rgb(22, 26, 37) });
    p.circle_stroke(pt(288.0, 208.0), sc(11.0), Stroke::new(1.2, Color32::from_rgb(74, 86, 114)));
    p.text(pt(288.0, 208.0), Align2::CENTER_CENTER, "★", FontId::proportional(sc(12.0)), if cap_on { Color32::BLACK } else { TEXT_MUTED });

    p.circle_filled(pt(352.0, 208.0), sc(11.0), if hom_on { ACCENT_VIOLET } else { Color32::from_rgb(22, 26, 37) });
    p.circle_stroke(pt(352.0, 208.0), sc(11.0), Stroke::new(1.4, ACCENT_CYAN));
    p.circle_filled(pt(352.0, 208.0), sc(4.0), if hom_on { Color32::WHITE } else { ACCENT_CYAN });

    // 9) Symmetrical Analog Sticks (live animated position + L3/R3 click highlight!)
    let draw_live_stick = |center: Pos2, x: u8, y: u8, clicked: bool| {
        let well_r = sc(34.0);
        let cap_r = sc(19.0);
        p.circle_filled(center, well_r, Color32::from_rgb(16, 19, 27));
        p.circle_stroke(
            center,
            well_r,
            Stroke::new(
                if clicked { 2.4 } else { 1.6 },
                if clicked { ACCENT_GREEN } else { Color32::from_rgb(64, 74, 98) },
            ),
        );
        let nx = ((x as f32) - 127.5) / 127.5;
        let ny = ((y as f32) - 127.5) / 127.5;
        let travel = well_r - cap_r * 0.72;
        let knob = Pos2::new(center.x + nx * travel, center.y + ny * travel);
        p.circle_filled(
            knob,
            cap_r,
            if clicked {
                Color32::from_rgb(38, 128, 96)
            } else {
                Color32::from_rgb(44, 52, 72)
            },
        );
        p.circle_stroke(
            knob,
            cap_r,
            Stroke::new(1.6, if clicked { ACCENT_GREEN } else { ACCENT_CYAN }),
        );
        p.circle_filled(knob, cap_r * 0.52, Color32::from_rgb(24, 29, 42));
    };

    draw_live_stick(pt(246.0, 264.0), st.lx, st.ly, (btns & BTN_L3) != 0);
    draw_live_stick(pt(394.0, 264.0), st.rx, st.ry, (btns & BTN_R3) != 0);

    // 10) Status LED dots at bottom center
    for i in 0..4 {
        let x = 302.0 + (i as f32) * 12.0;
        let c = if i == 0 {
            ACCENT_CYAN
        } else {
            Color32::from_rgb(42, 52, 72)
        };
        p.circle_filled(pt(x, 282.0), sc(3.0), c);
    }
}

/// Live artificial horizon / tilt indicator driven by 3-axis accelerometer + yaw arc.
fn draw_horizon_gauge(ui: &mut egui::Ui, ax: f32, ay: f32, az: f32, yaw_dps: f32) {
    ui.vertical_centered(|ui| {
        let size = 122.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let p = ui.painter();
        let center = rect.center();
        let r = size * 0.46;

        // Background sphere
        p.circle_filled(center, r, Color32::from_rgb(13, 17, 26));
        p.circle_stroke(center, r, Stroke::new(1.6, Color32::from_rgb(50, 60, 84)));

        // Compute roll and pitch angles from gravity vector (flat = ay ~ +1g)
        let roll_rad = ax.atan2(ay.max(0.05)).clamp(-1.1, 1.1);
        let pitch_rad = az.atan2((ax * ax + ay * ay).sqrt().max(0.05)).clamp(-1.0, 1.0);
        let pitch_px = pitch_rad * (r * 0.65);

        let cos_r = roll_rad.cos();
        let sin_r = roll_rad.sin();
        let half_len = r * 0.78;
        let mid = Pos2::new(center.x - sin_r * pitch_px, center.y + cos_r * pitch_px);
        let p1 = Pos2::new(mid.x - cos_r * half_len, mid.y - sin_r * half_len);
        let p2 = Pos2::new(mid.x + cos_r * half_len, mid.y + sin_r * half_len);

        // Horizon line
        p.line_segment([p1, p2], Stroke::new(2.4, ACCENT_CYAN));
        // Center reticle
        p.circle_stroke(center, 5.0, Stroke::new(1.5, ACCENT_AMBER));
        p.line_segment(
            [Pos2::new(center.x - 14.0, center.y), Pos2::new(center.x - 6.0, center.y)],
            Stroke::new(1.5, ACCENT_AMBER),
        );
        p.line_segment(
            [Pos2::new(center.x + 6.0, center.y), Pos2::new(center.x + 14.0, center.y)],
            Stroke::new(1.5, ACCENT_AMBER),
        );

        // Yaw indicator dot along the top rim
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
        .selected_text(action.label())
        .width(250.0)
        .show_ui(ui, |ui| {
            for &opt in PaddleAction::ALL {
                ui.selectable_value(action, opt, opt.label());
            }
        });
    ui.label(if pressed {
        RichText::new("PULSADO").color(ACCENT_GREEN).strong()
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
