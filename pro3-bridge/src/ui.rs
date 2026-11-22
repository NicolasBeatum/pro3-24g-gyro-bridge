use crate::bridge::{BridgeControl, BridgeStatus};
use crate::config::*;
use crate::protocol::*;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, RichText, Stroke, Vec2};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

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
                    .fill(Color32::from_rgb(18, 20, 26))
                    .inner_margin(egui::Margin::symmetric(16, 12)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            RichText::new("8BitDo Pro 3 · 2.4 GHz Bridge")
                                .size(19.0)
                                .strong()
                                .color(Color32::from_rgb(240, 244, 252)),
                        );
                        let dot_color = if status.connected {
                            Color32::from_rgb(56, 217, 134)
                        } else if status.running {
                            Color32::from_rgb(250, 189, 47)
                        } else {
                            Color32::from_rgb(160, 165, 178)
                        };
                        ui.horizontal(|ui| {
                            let (rect, _) = ui.allocate_exact_size(Vec2::splat(9.0), egui::Sense::hover());
                            ui.painter().circle_filled(rect.center(), 4.5, dot_color);
                            ui.label(
                                RichText::new(&status.state_text)
                                    .size(12.5)
                                    .color(Color32::from_rgb(180, 188, 204)),
                            );
                        });
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let is_running = self.ctrl.running.load(Ordering::Relaxed);
                        let btn_text = if is_running { "⏹ Detener" } else { "▶ Iniciar" };
                        let btn_fill = if is_running {
                            Color32::from_rgb(180, 56, 66)
                        } else {
                            Color32::from_rgb(40, 152, 96)
                        };
                        if ui
                            .add(
                                egui::Button::new(RichText::new(btn_text).strong().color(Color32::WHITE))
                                    .fill(btn_fill)
                                    .min_size(Vec2::new(94.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.running.store(!is_running, Ordering::Relaxed);
                        }

                        if ui
                            .add_enabled(
                                status.connected,
                                egui::Button::new("📳 Vibrar").min_size(Vec2::new(78.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.request_test_rumble.store(true, Ordering::Relaxed);
                        }

                        if ui
                            .add_enabled(
                                status.connected,
                                egui::Button::new("🎯 Calibrar").min_size(Vec2::new(84.0, 30.0)),
                            )
                            .clicked()
                        {
                            self.ctrl.request_calibrate.store(true, Ordering::Relaxed);
                        }
                    });
                });

                ui.add_space(8.0);
                // Live telemetry pills
                ui.horizontal_wrapped(|ui| {
                    status_pill(
                        ui,
                        &format!("{:.0} Hz", status.rate_hz),
                        if status.rate_hz > 150.0 {
                            Color32::from_rgb(30, 88, 62)
                        } else {
                            Color32::from_rgb(44, 48, 58)
                        },
                    );
                    status_pill(
                        ui,
                        &format!("🔋 {}%", status.battery),
                        if status.battery >= 25 {
                            Color32::from_rgb(35, 62, 92)
                        } else {
                            Color32::from_rgb(130, 50, 45)
                        },
                    );
                    status_pill(
                        ui,
                        &format!("🎮 {}", status.vigem_status),
                        if status.vigem_ok {
                            Color32::from_rgb(46, 56, 112)
                        } else {
                            Color32::from_rgb(66, 52, 42)
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
                    status_pill(ui, &dsu_txt, Color32::from_rgb(44, 48, 58));
                });
            });

        // Navigation Tabs
        egui::TopBottomPanel::top("tabs")
            .frame(
                egui::Frame::default()
                    .fill(Color32::from_rgb(24, 27, 34))
                    .inner_margin(egui::Margin::symmetric(16, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.tab, Tab::Dashboard, "  Panel Principal  ");
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Paddles,
                        "  Botones Extra (L4/R4/PL/PR)  ",
                    );
                    ui.selectable_value(
                        &mut self.tab,
                        Tab::Advanced,
                        "  Gyro PC & Emuladores  ",
                    );
                });
            });

        // Main Body
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(Color32::from_rgb(15, 17, 22))
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

        // Card 1: Quick Output Config
        card(ui, "Configuración de Salida", |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("Mando virtual:");
                ui.selectable_value(
                    &mut self.cfg.pad_mode,
                    PadMode::Ds4NativeGyro,
                    "DualShock 4 (Gyro Nativo Steam)",
                );
                ui.selectable_value(&mut self.cfg.pad_mode, PadMode::Xbox360, "Xbox 360");
                ui.selectable_value(&mut self.cfg.pad_mode, PadMode::None, "Solo DSU");
            });

            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                ui.label("Mapa A/B/X/Y:");
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::XboxPs, "Xbox / PS");
                ui.selectable_value(&mut self.cfg.layout, ButtonLayout::Nintendo, "Nintendo");
                ui.separator();
                ui.checkbox(&mut self.cfg.dsu_enabled, "Servidor DSU (Emuladores)");
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

        ui.add_space(10.0);

        // Card 2: Sticks, Triggers & Motion live visualizer
        ui.columns(2, |cols| {
            card(&mut cols[0], "Sticks y Gatillos Analógicos", |ui| {
                ui.horizontal(|ui| {
                    draw_stick(ui, "Stick Izq", st.lx, st.ly, (st.btns & BTN_L3) != 0);
                    ui.add_space(14.0);
                    draw_stick(ui, "Stick Der", st.rx, st.ry, (st.btns & BTN_R3) != 0);
                });
                ui.add_space(10.0);
                draw_bar(
                    ui,
                    &format!("L2  {:3}", st.l2),
                    st.l2 as f32 / 255.0,
                    Color32::from_rgb(76, 156, 255),
                );
                ui.add_space(4.0);
                draw_bar(
                    ui,
                    &format!("R2  {:3}", st.r2),
                    st.r2 as f32 / 255.0,
                    Color32::from_rgb(255, 118, 92),
                );
                if status.rumble_l > 0 || status.rumble_r > 0 {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(format!(
                            "Vibración activa: Izq {} · Der {}",
                            status.rumble_l, status.rumble_r
                        ))
                        .size(12.0)
                        .color(Color32::from_rgb(255, 198, 92)),
                    );
                }
            });

            card(&mut cols[1], "Giroscopio y Acelerómetro (200 Hz)", |ui| {
                let gx = st.gx_dps - status.gyro_bias[0];
                let gy = st.gy_dps - status.gyro_bias[1];
                let gz = st.gz_dps - status.gyro_bias[2];
                let pitch = -gy;
                let yaw = gz;
                let roll = -gx;

                draw_signed_bar(ui, "Pitch", pitch, 250.0, "°/s");
                ui.add_space(4.0);
                draw_signed_bar(ui, "Yaw  ", yaw, 250.0, "°/s");
                ui.add_space(4.0);
                draw_signed_bar(ui, "Roll ", roll, 250.0, "°/s");

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);
                ui.label(
                    RichText::new(format!(
                        "Acelerómetro (g):  X {:+.2}   Y {:+.2}   Z {:+.2}",
                        -st.ay_g, st.az_g, -st.ax_g
                    ))
                    .monospace()
                    .size(12.0)
                    .color(Color32::from_rgb(175, 190, 212)),
                );
                ui.label(
                    RichText::new(format!(
                        "Offset gyro: ({:+.2}, {:+.2}, {:+.2}) °/s · Recalibrar: Minus + Plus",
                        status.gyro_bias[0], status.gyro_bias[1], status.gyro_bias[2]
                    ))
                    .size(11.5)
                    .color(Color32::from_rgb(130, 140, 158)),
                );
            });
        });

        ui.add_space(10.0);

        // Card 3: Live Buttons Monitor
        card(ui, "Botones en Vivo", |ui| {
            let defs: &[(&str, u32)] = &[
                ("A / ✕", BTN_SOUTH),
                ("B / ○", BTN_EAST),
                ("X / □", BTN_WEST),
                ("Y / △", BTN_NORTH),
                ("↑", BTN_DUP),
                ("↓", BTN_DDOWN),
                ("←", BTN_DLEFT),
                ("→", BTN_DRIGHT),
                ("L1", BTN_L1),
                ("R1", BTN_R1),
                ("L2", BTN_L2_DIG),
                ("R2", BTN_R2_DIG),
                ("L3", BTN_L3),
                ("R3", BTN_R3),
                ("Minus", BTN_MINUS),
                ("Plus", BTN_PLUS),
                ("Home", BTN_HOME),
                ("Star", BTN_CAPTURE),
                ("P1", BTN_P1),
                ("P2", BTN_P2),
                ("P3", BTN_P3),
                ("P4", BTN_P4),
            ];
            ui.horizontal_wrapped(|ui| {
                for &(name, mask) in defs {
                    let active = (st.btns & mask) != 0;
                    let fill = if active {
                        Color32::from_rgb(66, 138, 255)
                    } else {
                        Color32::from_rgb(32, 36, 46)
                    };
                    let tc = if active {
                        Color32::WHITE
                    } else {
                        Color32::from_rgb(150, 158, 174)
                    };
                    egui::Frame::default()
                        .fill(fill)
                        .corner_radius(CornerRadius::same(5))
                        .inner_margin(egui::Margin::symmetric(8, 4))
                        .show(ui, |ui| {
                            ui.label(RichText::new(name).size(12.0).color(tc));
                        });
                }
            });
        });
    }

    fn ui_paddles(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let btns = status.telemetry.btns;
        card(ui, "Mapeo de Botones Extra (L4 / R4 / PL / PR)", |ui| {
            ui.label(
                RichText::new(
                    "Presioná cada botón trasero/extra en tu 8BitDo Pro 3 para ver qué indicador se enciende (P1..P4) y asignale la acción que quieras.",
                )
                .color(Color32::from_rgb(180, 188, 204)),
            );
            ui.add_space(6.0);
            ui.label(
                RichText::new(format!("Máscara raw actual: 0x{:08X}", btns))
                    .monospace()
                    .size(12.0)
                    .color(Color32::from_rgb(135, 190, 255)),
            );
            ui.add_space(10.0);

            egui::Grid::new("paddle_grid")
                .num_columns(3)
                .spacing([14.0, 10.0])
                .show(ui, |ui| {
                    paddle_row(ui, "Extra P1 (bit 25)", (btns & BTN_P1) != 0, "p1", &mut self.cfg.paddle_p1);
                    paddle_row(ui, "Extra P2 (bit 26)", (btns & BTN_P2) != 0, "p2", &mut self.cfg.paddle_p2);
                    paddle_row(ui, "Extra P3 (bit 21)", (btns & BTN_P3) != 0, "p3", &mut self.cfg.paddle_p3);
                    paddle_row(ui, "Extra P4 (bit 30)", (btns & BTN_P4) != 0, "p4", &mut self.cfg.paddle_p4);
                });

            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Nota: Si ya mapeaste L4/R4/PL/PR desde el propio mando o Ultimate Software a otro botón (ej. A o B), el mando seguirá enviando ese botón normalmente.",
                )
                .size(11.5)
                .color(Color32::from_rgb(140, 148, 165)),
            );
        });
    }

    fn ui_advanced(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        card(ui, "Giroscopio Directo para PC (opcional fuera de Steam Input)", |ui| {
            ui.label(
                RichText::new(
                    "En modo DualShock 4, dejalo en 'Desactivado' para que Steam Input maneje el giroscopio nativamente sin conflictos. Usá Mouse o Stick Derecho solo en juegos fuera de Steam o en modo Xbox 360.",
                )
                .size(12.0)
                .color(Color32::from_rgb(175, 184, 200)),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Modo:");
                ui.selectable_value(&mut self.cfg.gyro_pc_mode, GyroPcMode::Off, "Desactivado (recomendado en DS4)");
                ui.selectable_value(&mut self.cfg.gyro_pc_mode, GyroPcMode::Mouse, "Gyro → Mouse");
                ui.selectable_value(&mut self.cfg.gyro_pc_mode, GyroPcMode::RightStick, "Gyro → Stick Derecho");
            });

            if self.cfg.gyro_pc_mode != GyroPcMode::Off {
                ui.add_space(8.0);
                ui.horizontal_wrapped(|ui| {
                    ui.label("Activar con:");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::LeftTrigger, "Mantener L2");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::RightTrigger, "Mantener R2");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::LeftBumper, "Mantener L1");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::RightBumper, "Mantener R1");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::AlwaysOn, "Siempre activo");
                    ui.selectable_value(&mut self.cfg.gyro_trigger, GyroTrigger::ToggleCapture, "Toggle (Star)");
                });
                ui.add_space(6.0);
                ui.add(egui::Slider::new(&mut self.cfg.gyro_sens, 0.2..=4.0).text("Sensibilidad"));
                ui.add(egui::Slider::new(&mut self.cfg.gyro_roll_mix, 0.0..=1.0).text("Mezcla Roll en eje X"));
                ui.add(egui::Slider::new(&mut self.cfg.gyro_deadzone, 0.0..=5.0).text("Zona muerta (°/s)"));
                ui.horizontal(|ui| {
                    ui.checkbox(&mut self.cfg.invert_x, "Invertir eje X");
                    ui.checkbox(&mut self.cfg.invert_y, "Invertir eje Y");
                    ui.label(if status.gyro_pc_active { " · [ACTIVO]" } else { " · [en espera]" });
                });
            }
        });

        ui.add_space(10.0);

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
                     • Volver al modo XInput de fábrica del dongle: cerrá esta app y desenchufá/re-enchufá el dongle.",
                )
                .size(12.0)
                .color(Color32::from_rgb(175, 184, 200)),
            );
        });
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
        Color32::from_rgb(56, 217, 134)
    } else {
        Color32::from_rgb(60, 66, 80)
    };
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
        ui.painter().circle_filled(rect.center(), 5.0, dot_color);
        ui.label(RichText::new(label).strong());
    });
    egui::ComboBox::from_id_salt(id)
        .selected_text(action.label())
        .width(240.0)
        .show_ui(ui, |ui| {
            for &opt in PaddleAction::ALL {
                ui.selectable_value(action, opt, opt.label());
            }
        });
    ui.label(if pressed { "PULSADO" } else { "" });
    ui.end_row();
}

fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(Color32::from_rgb(24, 27, 35))
        .stroke(Stroke::new(1.0, Color32::from_rgb(42, 47, 60)))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .size(14.0)
                    .strong()
                    .color(Color32::from_rgb(228, 234, 246)),
            );
            ui.add_space(6.0);
            add_contents(ui);
        });
}

fn status_pill(ui: &mut egui::Ui, text: &str, bg: Color32) {
    egui::Frame::default()
        .fill(bg)
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::symmetric(10, 4))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(12.0).color(Color32::from_rgb(235, 240, 248)));
        });
}

fn draw_stick(ui: &mut egui::Ui, label: &str, x: u8, y: u8, clicked: bool) {
    ui.vertical_centered(|ui| {
        let size = 76.0;
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), egui::Sense::hover());
        let p = ui.painter();
        let center = rect.center();
        let r = size * 0.46;
        p.circle_filled(center, r, Color32::from_rgb(16, 18, 24));
        p.circle_stroke(
            center,
            r,
            Stroke::new(
                1.5,
                if clicked {
                    Color32::from_rgb(66, 156, 255)
                } else {
                    Color32::from_rgb(65, 72, 90)
                },
            ),
        );
        p.line_segment(
            [Pos2::new(center.x - r, center.y), Pos2::new(center.x + r, center.y)],
            Stroke::new(0.8, Color32::from_rgb(40, 45, 58)),
        );
        p.line_segment(
            [Pos2::new(center.x, center.y - r), Pos2::new(center.x, center.y + r)],
            Stroke::new(0.8, Color32::from_rgb(40, 45, 58)),
        );
        let nx = ((x as f32) - 127.5) / 127.5;
        let ny = ((y as f32) - 127.5) / 127.5;
        let dot = Pos2::new(center.x + nx * (r - 6.0), center.y + ny * (r - 6.0));
        p.circle_filled(
            dot,
            6.5,
            if clicked {
                Color32::from_rgb(90, 220, 150)
            } else {
                Color32::from_rgb(80, 160, 255)
            },
        );
        ui.label(RichText::new(format!("{label} ({x},{y})")).size(11.0).color(Color32::from_rgb(155, 163, 180)));
    });
}

fn draw_bar(ui: &mut egui::Ui, label: &str, frac: f32, color: Color32) {
    ui.horizontal(|ui| {
        ui.label(RichText::new(label).monospace().size(12.0));
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(16, 18, 24));
        let w = rect.width() * frac.clamp(0.0, 1.0);
        if w > 0.5 {
            p.rect_filled(
                Rect::from_min_size(rect.min, Vec2::new(w, rect.height())),
                CornerRadius::same(4),
                color,
            );
        }
    });
}

fn draw_signed_bar(ui: &mut egui::Ui, label: &str, value: f32, max_abs: f32, unit: &str) {
    ui.horizontal(|ui| {
        ui.label(
            RichText::new(format!("{label} {:+6.1} {unit}", value))
                .monospace()
                .size(12.0),
        );
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 14.0), egui::Sense::hover());
        let p = ui.painter();
        p.rect_filled(rect, CornerRadius::same(4), Color32::from_rgb(16, 18, 24));
        let mid_x = rect.center().x;
        p.line_segment(
            [Pos2::new(mid_x, rect.top()), Pos2::new(mid_x, rect.bottom())],
            Stroke::new(1.0, Color32::from_rgb(70, 78, 95)),
        );
        let norm = (value / max_abs).clamp(-1.0, 1.0);
        let half = rect.width() * 0.5;
        let fill_rect = if norm >= 0.0 {
            Rect::from_min_max(Pos2::new(mid_x, rect.top()), Pos2::new(mid_x + norm * half, rect.bottom()))
        } else {
            Rect::from_min_max(Pos2::new(mid_x + norm * half, rect.top()), Pos2::new(mid_x, rect.bottom()))
        };
        if fill_rect.width() > 0.5 {
            p.rect_filled(fill_rect, CornerRadius::same(3), Color32::from_rgb(80, 185, 255));
        }
    });
}
