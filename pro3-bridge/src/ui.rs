use crate::bridge::{BridgeControl, BridgeStatus};
use crate::config::*;
use crate::protocol::*;
use crate::setup::{self, InstallState};
use eframe::egui::{
    self, Align, Align2, Color32, CornerRadius, CursorIcon, FontId, Layout, Painter, Pos2, Rect,
    RichText, Sense, Shape, Stroke, StrokeKind, Vec2,
};
use std::f32::consts::{FRAC_PI_2, TAU};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tray_icon::TrayIcon;

pub const PRO3_SVG: &[u8] = include_bytes!("../../assets/8bitdo_pro3.svg");
const REPO_URL: &str = "https://github.com/NicolasBeatum/pro3-24g-gyro-bridge";

// ── Design tokens ──────────────────────────────────────────────────────────
// Deep matte canvas, soft cards, ultra-subtle 1px strokes.
pub const BG_CANVAS: Color32 = Color32::from_rgb(12, 14, 20); // #0C0E14
pub const BG_CARD: Color32 = Color32::from_rgb(19, 23, 34); // #131722
pub const BG_INSET: Color32 = Color32::from_rgb(14, 17, 25);
pub const BG_RAISED: Color32 = Color32::from_rgb(34, 41, 58);
pub const BG_ELEVATED: Color32 = Color32::from_rgb(22, 26, 38);
pub const STROKE: Color32 = Color32::from_rgb(34, 40, 56); // #222838
pub const STROKE_STRONG: Color32 = Color32::from_rgb(50, 59, 82);
const TEXT_PRIMARY: Color32 = Color32::from_rgb(232, 236, 244);
const TEXT_SECONDARY: Color32 = Color32::from_rgb(160, 169, 188);
const TEXT_MUTED: Color32 = Color32::from_rgb(104, 113, 134);
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

#[derive(Clone, Copy)]
enum ModalChoice {
    Tray,
    Exit,
    Cancel,
}

/// Bilingual string picker.
fn t(lang: Language, es: &'static str, en: &'static str) -> &'static str {
    match lang {
        Language::Es => es,
        Language::En => en,
    }
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
    show_setup_modal: bool,
    vigem_installed: bool,
    dongle_present: bool,
    last_setup_probe: Instant,
    install_state: Arc<Mutex<InstallState>>,
}

impl Pro3App {
    pub fn new(
        ctrl: Arc<BridgeControl>,
        cfg: BridgeConfig,
        tray: Option<TrayIcon>,
        force_exit: Arc<AtomicBool>,
    ) -> Self {
        let vigem_installed = setup::is_vigem_installed();
        let dongle_present = setup::is_dongle_present();
        let show_setup_modal = !vigem_installed && cfg.pad_mode != PadMode::None;
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
            show_setup_modal,
            vigem_installed,
            dongle_present,
            last_setup_probe: Instant::now(),
            install_state: Arc::new(Mutex::new(InstallState::Idle)),
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
        let l = self.cfg.lang;
        let mut choice: Option<ModalChoice> = None;

        let modal = egui::Modal::new(egui::Id::new("close_behavior_modal"))
            .backdrop_color(Color32::from_black_alpha(160))
            .frame(
                egui::Frame::default()
                    .fill(BG_ELEVATED)
                    .stroke(Stroke::new(1.0, STROKE_STRONG))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(430.0);
                ui.spacing_mut().item_spacing.y = 6.0;
                ui.label(
                    RichText::new(t(l, "¿Qué quieres hacer al cerrar?", "What should happen when you close?"))
                        .size(16.0)
                        .strong()
                        .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(t(
                        l,
                        "Elige qué hace la ventana al pulsar X.",
                        "Choose what the window does when you click X.",
                    ))
                    .size(12.0)
                    .color(TEXT_MUTED),
                );
                ui.add_space(10.0);

                option_tile(
                    ui,
                    ACCENT_CYAN,
                    t(l, "Minimizar a la bandeja", "Minimize to Tray"),
                    t(
                        l,
                        "Oculta la ventana en la bandeja del sistema (junto al reloj) y mantiene el giroscopio, el DualShock 4 virtual y DSU funcionando en segundo plano.",
                        "Hides the window to the system tray (next to the clock) and keeps the gyro, virtual DualShock 4 and DSU running in the background.",
                    ),
                    TEXT_SECONDARY,
                );
                ui.add_space(2.0);
                option_tile(
                    ui,
                    ACCENT_AMBER,
                    t(l, "Cerrar completamente", "Exit Completely"),
                    if self.cfg.restore_xinput_on_exit {
                        t(
                            l,
                            "Detiene el puente y devuelve el dongle al modo XInput de fábrica (~6 s). El mando se apagará un momento: vuelve a encenderlo y funcionará con normalidad, sin desenchufar nada.",
                            "Stops the bridge and returns the dongle to stock XInput mode (~6 s). The controller turns off briefly — just turn it back on and it works as usual, no unplugging needed.",
                        )
                    } else {
                        t(
                            l,
                            "Detiene el puente. La restauración automática está desactivada: desconecta y vuelve a conectar el dongle USB para volver a XInput.",
                            "Stops the bridge. Auto-restore is off, so unplug and replug the USB dongle to return to stock XInput.",
                        )
                    },
                    Color32::from_rgb(232, 196, 120),
                );

                ui.add_space(10.0);
                ui.checkbox(
                    &mut self.remember_close_choice,
                    RichText::new(t(
                        l,
                        "Recordar mi elección (puedes cambiarla en Avanzado)",
                        "Remember my choice (you can change it in Advanced)",
                    ))
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(14.0);

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if solid_button(
                        ui,
                        t(l, "Minimizar a la bandeja", "Minimize to Tray"),
                        Color32::from_rgb(14, 116, 178),
                        Color32::WHITE,
                        Stroke::NONE,
                    )
                    .clicked()
                    {
                        choice = Some(ModalChoice::Tray);
                    }
                    if solid_button(
                        ui,
                        t(l, "Cerrar la app", "Exit App"),
                        Color32::from_rgb(58, 24, 32),
                        Color32::from_rgb(255, 176, 186),
                        Stroke::new(1.0, with_alpha(ACCENT_CORAL, 110)),
                    )
                    .clicked()
                    {
                        choice = Some(ModalChoice::Exit);
                    }
                    if ui
                        .add(
                            egui::Button::new(RichText::new(t(l, "Cancelar", "Cancel")).size(13.0))
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(84.0, 32.0)),
                        )
                        .clicked()
                    {
                        choice = Some(ModalChoice::Cancel);
                    }
                });
            });

        if choice.is_none() && modal.should_close() {
            choice = Some(ModalChoice::Cancel);
        }

        match choice {
            Some(ModalChoice::Tray) => {
                if self.remember_close_choice {
                    self.cfg.close_action = CloseAction::MinimizeToTray;
                    self.sync_config();
                }
                self.show_close_modal = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
            Some(ModalChoice::Exit) => {
                if self.remember_close_choice {
                    self.cfg.close_action = CloseAction::Exit;
                    self.sync_config();
                }
                self.show_close_modal = false;
                self.force_exit.store(true, Ordering::Relaxed);
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Some(ModalChoice::Cancel) => self.show_close_modal = false,
            None => {}
        }
    }

    fn ui_header(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let l = self.cfg.lang;
        let time = ui.input(|i| i.time) as f32;
        ui.spacing_mut().item_spacing.y = 0.0;

        // ── Row 1: identity + status on the left, actions on the right ──
        let row_w = ui.available_width();
        ui.allocate_ui_with_layout(Vec2::new(row_w, 44.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_height(44.0);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 3.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(
                        RichText::new("8BitDo Pro 3")
                            .size(17.0)
                            .strong()
                            .color(TEXT_PRIMARY),
                    );
                    ui.label(RichText::new("Gyro Bridge").size(17.0).color(TEXT_MUTED));
                    ui.add_space(4.0);
                    egui::Frame::default()
                        .stroke(Stroke::new(1.0, STROKE_STRONG))
                        .corner_radius(CornerRadius::same(10))
                        .inner_margin(egui::Margin::symmetric(7, 1))
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new("2.4 GHz · 200 Hz")
                                    .size(10.5)
                                    .color(TEXT_SECONDARY),
                            );
                        });
                });
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 7.0;
                    let dot_color = if status.connected {
                        ACCENT_GREEN
                    } else if status.running {
                        ACCENT_AMBER
                    } else {
                        TEXT_MUTED
                    };
                    let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    let pulse = if status.connected || !status.running {
                        0.55
                    } else {
                        (time * 3.2).sin() * 0.5 + 0.5
                    };
                    let p = ui.painter();
                    p.circle_filled(
                        rect.center(),
                        3.5 + 2.5 * pulse,
                        with_alpha(dot_color, (30.0 + 40.0 * pulse) as u8),
                    );
                    p.circle_filled(rect.center(), 3.4, dot_color);
                    let state_txt = status.conn_state.label(l);
                    let r = ui.label(
                        RichText::new(ellipsize(&state_txt, 60))
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                    );
                    if state_txt.chars().count() > 60 {
                        r.on_hover_text(state_txt);
                    }
                });
            });

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                let is_running = self.ctrl.running.load(Ordering::Relaxed);
                let (txt, fill, fg, stroke, tip) = if is_running {
                    (
                        t(l, "Detener", "Stop"),
                        Color32::from_rgb(52, 22, 30),
                        Color32::from_rgb(255, 170, 182),
                        Stroke::new(1.0, with_alpha(ACCENT_CORAL, 100)),
                        t(
                            l,
                            "Detiene el puente y devuelve el dongle a XInput de fábrica (~6 s; luego vuelve a encender el mando)",
                            "Stops the bridge and returns the dongle to stock XInput (~6 s; then turn the controller back on)",
                        ),
                    )
                } else {
                    (
                        t(l, "Iniciar", "Start"),
                        Color32::from_rgb(16, 146, 104),
                        Color32::WHITE,
                        Stroke::NONE,
                        t(
                            l,
                            "Activa la telemetría de 200 Hz e inicia el puente (HID → ViGEmBus / DSU)",
                            "Enables 200 Hz telemetry and starts the bridge (HID → ViGEmBus / DSU)",
                        ),
                    )
                };
                if solid_button(ui, txt, fill, fg, stroke)
                    .on_hover_text(tip)
                    .clicked()
                {
                    self.ctrl.running.store(!is_running, Ordering::Relaxed);
                }

                let need_ctrl = t(l, "Primero conecta el mando", "Connect the controller first");
                if ghost_button(ui, t(l, "Vibrar", "Rumble"), status.connected)
                    .on_hover_text(t(
                        l,
                        "Envía un breve pulso de vibración de prueba",
                        "Sends a short test rumble pulse",
                    ))
                    .on_disabled_hover_text(need_ctrl)
                    .clicked()
                {
                    self.ctrl.request_test_rumble.store(true, Ordering::Relaxed);
                }
                if ghost_button(ui, t(l, "Calibrar", "Calibrate"), status.connected)
                    .on_hover_text(t(
                        l,
                        "Recalibra el sesgo del giroscopio (deja el mando quieto). Atajo en el mando: mantén Minus + Plus",
                        "Recalibrates the gyro bias (keep the controller still). Controller shortcut: hold Minus + Plus",
                    ))
                    .on_disabled_hover_text(need_ctrl)
                    .clicked()
                {
                    self.ctrl.request_calibrate.store(true, Ordering::Relaxed);
                }

                ui.add_space(8.0);
                segmented(
                    ui,
                    &mut self.cfg.lang,
                    &[(Language::En, "EN"), (Language::Es, "ES")],
                    &["English", "Español"],
                );
            });
        });

        ui.add_space(10.0);

        // ── Row 2: quiet single-line status strip ──
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            let hz_col = if status.rate_hz > 150.0 {
                ACCENT_GREEN
            } else if status.rate_hz > 1.0 {
                ACCENT_AMBER
            } else {
                TEXT_SECONDARY
            };
            strip_item(ui, t(l, "Frecuencia", "Rate"), &format!("{:.0} Hz", status.rate_hz), hz_col);
            strip_sep(ui);

            ui.label(RichText::new(t(l, "Batería", "Battery")).size(11.5).color(TEXT_MUTED));
            let bat_known = status.connected || status.battery > 0;
            battery_glyph(ui, status.battery, bat_known);
            let bat_col = if !bat_known {
                TEXT_SECONDARY
            } else if status.battery < 25 {
                ACCENT_CORAL
            } else {
                TEXT_PRIMARY
            };
            let bat_txt = if bat_known {
                format!("{}%", status.battery)
            } else {
                "—".to_string()
            };
            ui.label(RichText::new(bat_txt).size(11.5).color(bat_col));
            strip_sep(ui);

            let (vigem_txt, vigem_col, vigem_full) = if let Some(err) = &status.vigem_error {
                (
                    t(l, "no disponible — instala ViGEmBus", "unavailable — install ViGEmBus").to_string(),
                    ACCENT_CORAL,
                    Some(err.clone()),
                )
            } else {
                let s: &str = match status.vigem_mode_active {
                    Some(PadMode::Ds4NativeGyro) => {
                        t(l, "DualShock 4 (giroscopio nativo en Steam)", "DualShock 4 (native Steam gyro)")
                    }
                    Some(PadMode::Xbox360) => "Xbox 360 (XInput)",
                    Some(PadMode::None) => t(l, "Solo DSU", "DSU Only"),
                    None => t(l, "inactivo", "inactive"),
                };
                let col = if status.vigem_ok { TEXT_PRIMARY } else { ACCENT_AMBER };
                (s.to_string(), col, None)
            };
            let r = strip_item(ui, "ViGEmBus", &vigem_txt, vigem_col);
            if let Some(full) = vigem_full {
                r.on_hover_text(full);
            }
            strip_sep(ui);

            let (dsu_txt, dsu_col, dsu_full) = if self.cfg.dsu_enabled {
                if let Some(err) = &status.dsu_error {
                    (ellipsize(err, 40), ACCENT_CORAL, Some(err.clone()))
                } else {
                    let cli = match (l, status.dsu_clients) {
                        (Language::Es, 1) => "cliente",
                        (Language::Es, _) => "clientes",
                        (Language::En, 1) => "client",
                        (Language::En, _) => "clients",
                    };
                    (
                        format!(":{} · {} {}", self.cfg.dsu_port, status.dsu_clients, cli),
                        if status.dsu_clients > 0 { ACCENT_CYAN } else { TEXT_PRIMARY },
                        None,
                    )
                }
            } else {
                (t(l, "desactivado", "off").to_string(), TEXT_MUTED, None)
            };
            let r = strip_item(ui, "DSU", &dsu_txt, dsu_col);
            if let Some(full) = dsu_full {
                r.on_hover_text(full);
            }
        });

        ui.add_space(4.0);

        // ── Row 3: underline navigation tabs (sit on the panel separator) ──
        let tabs = [
            (Tab::Dashboard, t(l, "Panel en vivo", "Live Dashboard")),
            (Tab::Paddles, t(l, "Botones traseros (PL / PR)", "Back Paddles (PL / PR)")),
            (Tab::Advanced, t(l, "Giroscopio en PC y avanzado", "PC Gyro & Advanced")),
        ];
        tab_bar(ui, &mut self.tab, &tabs);
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
        let prev_cfg_header = self.cfg.clone();

        egui::TopBottomPanel::top("header")
            .show_separator_line(true)
            .frame(
                egui::Frame::default()
                    .fill(BG_CANVAS)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 14,
                        bottom: 0,
                    }),
            )
            .show(ctx, |ui| self.ui_header(ui, &status));

        if self.cfg != prev_cfg_header {
            self.sync_config();
        }

        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(BG_CANVAS)
                    .inner_margin(egui::Margin::symmetric(20, 16)),
            )
            .show(ctx, |ui| {
                let view_h = ui.available_height();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let prev_cfg = self.cfg.clone();
                        match self.tab {
                            Tab::Dashboard => self.ui_dashboard(ui, &status, view_h),
                            Tab::Paddles => self.ui_paddles(ui, &status),
                            Tab::Advanced => self.ui_advanced(ui, &status),
                        }
                        if self.cfg != prev_cfg {
                            self.sync_config();
                        }
                    });
            });

        self.render_close_modal(ctx);
        self.render_setup_modal(ctx);
    }
}

impl Pro3App {
    fn refresh_setup_probes(&mut self) {
        self.vigem_installed = setup::is_vigem_installed();
        self.dongle_present = setup::is_dongle_present();
        self.last_setup_probe = Instant::now();
    }

    fn render_setup_modal(&mut self, ctx: &egui::Context) {
        if !self.show_setup_modal {
            return;
        }
        if self.last_setup_probe.elapsed() >= Duration::from_millis(1200) {
            self.refresh_setup_probes();
        }

        let l = self.cfg.lang;
        let inst_state = self
            .install_state
            .lock()
            .map(|s| s.clone())
            .unwrap_or(InstallState::Idle);
        let busy = matches!(
            inst_state,
            InstallState::Downloading
                | InstallState::Verifying
                | InstallState::RunningInstaller
        );

        let mut close_now = false;
        let mut switch_dsu_only = false;

        let modal = egui::Modal::new(egui::Id::new("steam_setup_modal"))
            .backdrop_color(Color32::from_black_alpha(165))
            .frame(
                egui::Frame::default()
                    .fill(BG_ELEVATED)
                    .stroke(Stroke::new(1.0, STROKE_STRONG))
                    .corner_radius(CornerRadius::same(14))
                    .inner_margin(egui::Margin::same(22)),
            )
            .show(ctx, |ui| {
                ui.set_width(470.0);
                ui.spacing_mut().item_spacing.y = 6.0;

                ui.label(
                    RichText::new(t(
                        l,
                        "Asistente de configuración para Steam",
                        "Steam & Driver Setup Assistant",
                    ))
                    .size(16.0)
                    .strong()
                    .color(TEXT_PRIMARY),
                );
                ui.label(
                    RichText::new(t(
                        l,
                        "Revisa los requisitos para usar el giroscopio nativo (DualShock 4) en Steam. Solo se configura una vez.",
                        "Check the requirements for native DualShock 4 gyro in Steam. This is a one-time setup.",
                    ))
                    .size(12.0)
                    .color(TEXT_MUTED),
                );
                ui.add_space(10.0);

                // Step 1: ViGEmBus driver
                egui::Frame::default()
                    .fill(BG_INSET)
                    .stroke(Stroke::new(
                        1.0,
                        if self.vigem_installed {
                            with_alpha(ACCENT_GREEN, 95)
                        } else {
                            with_alpha(ACCENT_AMBER, 110)
                        },
                    ))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let dot = if self.vigem_installed { ACCENT_GREEN } else { ACCENT_AMBER };
                            let (r, _) = ui.allocate_exact_size(Vec2::new(8.0, 16.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.5, dot);
                            ui.label(
                                RichText::new(format!("1. Driver ViGEmBus (v{})", setup::VIGEM_VERSION))
                                    .size(13.0)
                                    .strong()
                                    .color(TEXT_PRIMARY),
                            );
                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                let (badge, col) = if self.vigem_installed {
                                    (t(l, "Instalado", "Installed"), ACCENT_GREEN)
                                } else {
                                    (t(l, "No instalado", "Missing"), ACCENT_AMBER)
                                };
                                ui.label(RichText::new(badge).size(11.5).strong().color(col));
                            });
                        });
                        ui.add_space(3.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(t(
                                    l,
                                    "Driver firmado oficial de Nefarius necesario para crear el DualShock 4 virtual en Steam (opcional si solo usas emuladores DSU).",
                                    "Official signed driver by Nefarius required to create the virtual DualShock 4 in Steam (optional for DSU-only emulators).",
                                ))
                                .size(12.0)
                                .color(TEXT_SECONDARY),
                            )
                            .wrap(),
                        );

                        if !self.vigem_installed {
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 8.0;
                                let btn_lbl = match &inst_state {
                                    InstallState::Downloading => t(l, "Descargando...", "Downloading..."),
                                    InstallState::Verifying => t(l, "Verificando firma...", "Verifying signature..."),
                                    InstallState::RunningInstaller => t(l, "Instalando...", "Installing..."),
                                    _ => t(l, "Instalar ViGEmBus", "Install ViGEmBus"),
                                };
                                if ui
                                    .add_enabled(
                                        !busy,
                                        egui::Button::new(
                                            RichText::new(btn_lbl)
                                                .size(12.5)
                                                .strong()
                                                .color(Color32::WHITE),
                                        )
                                        .fill(Color32::from_rgb(14, 116, 178))
                                        .corner_radius(CornerRadius::same(8))
                                        .min_size(Vec2::new(148.0, 30.0)),
                                    )
                                    .on_hover_text(t(
                                        l,
                                        "Descarga el instalador oficial de GitHub, verifica su firma digital de Windows y lo ejecuta",
                                        "Downloads the official GitHub installer, verifies its Windows signature, and launches it",
                                    ))
                                    .clicked()
                                {
                                    setup::spawn_vigem_installer(self.install_state.clone());
                                }
                                if ghost_button(
                                    ui,
                                    t(l, "Abrir GitHub oficial", "Open official GitHub"),
                                    true,
                                )
                                .clicked()
                                {
                                    setup::open_url(setup::VIGEM_RELEASE_URL);
                                }
                            });
                            if let InstallState::Error(err) = &inst_state {
                                ui.add_space(4.0);
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(err).size(11.5).color(ACCENT_CORAL),
                                    )
                                    .wrap(),
                                );
                            } else if matches!(inst_state, InstallState::RunningInstaller) {
                                ui.add_space(4.0);
                                ui.label(
                                    RichText::new(t(
                                        l,
                                        "Completa el instalador de ViGEmBus en la ventana de Windows; se detectará automáticamente al terminar.",
                                        "Finish the ViGEmBus installer window; it will be detected automatically once done.",
                                    ))
                                    .size(11.5)
                                    .color(ACCENT_CYAN),
                                );
                            }
                        }
                    });

                ui.add_space(2.0);

                // Step 2: Steam Input setting
                option_tile(
                    ui,
                    ACCENT_CYAN,
                    t(l, "2. Activar Steam Input para PlayStation", "2. Enable PlayStation Steam Input"),
                    t(
                        l,
                        "En Steam → Parámetros → Mando, activa «Steam Input para mandos de PlayStation». Luego pulsa Iniciar en esta app y activa el giroscopio en el juego.",
                        "In Steam → Settings → Controller, enable \"Steam Input for PlayStation controllers\". Then click Start in this app and enable gyro in your game.",
                    ),
                    TEXT_SECONDARY,
                );

                ui.add_space(2.0);

                // Step 3: Dongle presence
                option_tile(
                    ui,
                    if self.dongle_present { ACCENT_GREEN } else { TEXT_MUTED },
                    if self.dongle_present {
                        t(l, "3. Dongle USB 2.4 GHz · Detectado", "3. 2.4 GHz USB Dongle · Detected")
                    } else {
                        t(l, "3. Dongle USB 2.4 GHz · Esperando conexión", "3. 2.4 GHz USB Dongle · Waiting")
                    },
                    t(
                        l,
                        "No necesitas HidHide ni drivers extra. Al pulsar Detener o cerrar la app, el dongle vuelve solo a XInput (~6 s).",
                        "No HidHide or extra drivers needed. Clicking Stop or exiting restores stock XInput automatically (~6 s).",
                    ),
                    TEXT_SECONDARY,
                );

                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if self.vigem_installed {
                        if solid_button(
                            ui,
                            t(l, "Listo · Empezar", "All Set · Continue"),
                            Color32::from_rgb(16, 146, 104),
                            Color32::WHITE,
                            Stroke::NONE,
                        )
                        .clicked()
                        {
                            close_now = true;
                        }
                    } else {
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new(t(l, "Ahora no", "Later")).size(13.0),
                                )
                                .corner_radius(CornerRadius::same(8))
                                .min_size(Vec2::new(88.0, 32.0)),
                            )
                            .clicked()
                        {
                            close_now = true;
                        }
                        if ghost_button(
                            ui,
                            t(l, "Solo emuladores (DSU)", "Emulators Only (DSU)"),
                            !busy,
                        )
                        .clicked()
                        {
                            switch_dsu_only = true;
                        }
                    }
                    if ghost_button(ui, t(l, "Re-comprobar", "Re-check"), !busy).clicked() {
                        self.refresh_setup_probes();
                    }
                });
            });

        if modal.should_close() && !busy {
            close_now = true;
        }
        if switch_dsu_only {
            self.cfg.pad_mode = PadMode::None;
            self.sync_config();
            self.show_setup_modal = false;
        } else if close_now {
            self.show_setup_modal = false;
        }
    }

    fn ui_dashboard(&mut self, ui: &mut egui::Ui, status: &BridgeStatus, view_h: f32) {
        let st = &status.telemetry;
        let l = self.cfg.lang;
        let gap = 12.0;

        // Compact alert strip if ViGEmBus is still missing while a virtual pad mode is selected
        let mut banner_h = 0.0;
        if !self.vigem_installed && self.cfg.pad_mode != PadMode::None {
            let b = egui::Frame::default()
                .fill(lerp_color(BG_CARD, ACCENT_AMBER, 0.08))
                .stroke(Stroke::new(1.0, with_alpha(ACCENT_AMBER, 100)))
                .corner_radius(CornerRadius::same(10))
                .inner_margin(egui::Margin::symmetric(14, 8))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(t(
                                l,
                                "ViGEmBus no está instalado: necesario para DualShock 4 / Xbox 360 en Steam.",
                                "ViGEmBus is not installed: required for virtual DualShock 4 / Xbox 360 in Steam.",
                            ))
                            .size(12.0)
                            .color(Color32::from_rgb(245, 212, 130)),
                        );
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        RichText::new(t(l, "Instalar ViGEmBus", "Install ViGEmBus"))
                                            .size(12.0)
                                            .strong()
                                            .color(Color32::WHITE),
                                    )
                                    .fill(Color32::from_rgb(14, 116, 178))
                                    .corner_radius(CornerRadius::same(7))
                                    .min_size(Vec2::new(124.0, 26.0)),
                                )
                                .clicked()
                            {
                                self.refresh_setup_probes();
                                self.show_setup_modal = true;
                            }
                        });
                    });
                });
            banner_h = b.response.rect.height() + gap;
            ui.add_space(gap);
        }

        // ── Output profile: compact settings bar ──
        let prof = card(ui, "", None, 0.0, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = Vec2::new(10.0, 8.0);
                field_label(ui, t(l, "Salida", "Output"));
                segmented(
                    ui,
                    &mut self.cfg.pad_mode,
                    &[
                        (
                            PadMode::Ds4NativeGyro,
                            t(l, "DualShock 4 · Giroscopio nativo", "DualShock 4 · Native gyro"),
                        ),
                        (PadMode::Xbox360, "Xbox 360"),
                        (PadMode::None, t(l, "Solo DSU", "DSU Only")),
                    ],
                    &[
                        t(
                            l,
                            "DualShock 4 virtual con giroscopio nativo para Steam Input",
                            "Virtual DualShock 4 with native gyro for Steam Input",
                        ),
                        t(l, "Mando Xbox 360 virtual (XInput, sin giroscopio)", "Virtual Xbox 360 pad (XInput, no gyro)"),
                        t(
                            l,
                            "Sin mando virtual: solo movimiento por DSU para emuladores",
                            "No virtual pad: DSU motion for emulators only",
                        ),
                    ],
                );
                ui.add_space(10.0);
                field_label(ui, t(l, "Botones", "Buttons"));
                segmented(
                    ui,
                    &mut self.cfg.layout,
                    &[(ButtonLayout::XboxPs, "Xbox / PS"), (ButtonLayout::Nintendo, "Nintendo")],
                    &[
                        t(l, "A abajo, B a la derecha (estándar Xbox / PlayStation)", "A at the bottom, B on the right (Xbox / PlayStation standard)"),
                        t(l, "Intercambia A/B y X/Y (estilo Nintendo)", "Swaps A/B and X/Y (Nintendo style)"),
                    ],
                );
            });
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                toggle(ui, &mut self.cfg.dsu_enabled, t(l, "DSU para emuladores (UDP)", "DSU for emulators (UDP)"));
                ui.add_space(14.0);
                toggle(ui, &mut self.cfg.rumble_enabled, t(l, "Vibración", "Rumble"));
                if self.cfg.rumble_enabled {
                    ui.spacing_mut().slider_width = 150.0;
                    ui.add(
                        egui::Slider::new(&mut self.cfg.rumble_strength, 0.1..=1.0)
                            .fixed_decimals(0)
                            .custom_formatter(|v, _| format!("{}%", (v * 100.0).round() as i32)),
                    )
                    .on_hover_text(t(l, "Intensidad de la vibración", "Rumble strength"));
                }
            });
        });

        ui.add_space(gap);

        let showcase_h = (view_h - prof.rect.height() - banner_h - gap - 2.0).clamp(330.0, 470.0);

        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            let total_w = ui.available_width();
            let right_w = (total_w * 0.38).clamp(270.0, 380.0);
            let left_w = (total_w - right_w - gap).max(380.0);

            // ── Controller showcase ──
            ui.allocate_ui_with_layout(
                Vec2::new(left_w, showcase_h),
                Layout::top_down(Align::Min),
                |ui| {
                    let meta = if status.connected {
                        (t(l, "En vivo", "Live"), ACCENT_GREEN)
                    } else {
                        (t(l, "Sin señal", "No signal"), TEXT_MUTED)
                    };
                    card(
                        ui,
                        t(l, "8BitDo Pro 3 · Vista en vivo", "8BitDo Pro 3 · Live View"),
                        Some(meta),
                        showcase_h,
                        |ui| {
                            let max_h = ui.available_height();
                            draw_pro3_controller(
                                ui,
                                self.pro3_tex.as_ref(),
                                st,
                                status.rumble_l,
                                status.rumble_r,
                                max_h,
                            );
                        },
                    );
                },
            );

            // ── Gyroscope & artificial horizon ──
            ui.allocate_ui_with_layout(
                Vec2::new(right_w, showcase_h),
                Layout::top_down(Align::Min),
                |ui| {
                    let hz = format!("{:.0} Hz", status.rate_hz);
                    let hz_col = if status.rate_hz > 150.0 { ACCENT_GREEN } else { TEXT_MUTED };
                    card(
                        ui,
                        t(l, "Giroscopio y orientación", "Gyroscope & Orientation"),
                        Some((&hz, hz_col)),
                        showcase_h,
                        |ui| {
                            let gx = st.gx_dps - status.gyro_bias[0];
                            let gy = st.gy_dps - status.gyro_bias[1];
                            let gz = st.gz_dps - status.gyro_bias[2];
                            let pitch_dps = -gy;
                            let yaw_dps = gz;
                            let roll_dps = -gx;

                            let gauge = (ui.available_height() - 158.0)
                                .clamp(120.0, 176.0)
                                .min(ui.available_width());
                            draw_horizon_gauge(ui, -st.ay_g, st.az_g, -st.ax_g, yaw_dps, gauge);
                            ui.spacing_mut().item_spacing.y = 4.0;
                            ui.add_space(6.0);
                            axis_bar(ui, "Pitch", pitch_dps, 250.0, ACCENT_CYAN);
                            axis_bar(ui, "Yaw", yaw_dps, 250.0, ACCENT_VIOLET);
                            axis_bar(ui, "Roll", roll_dps, 250.0, ACCENT_GREEN);

                            ui.add_space(4.0);
                            hairline(ui);
                            ui.add_space(4.0);

                            kv_mono(
                                ui,
                                "Accel",
                                &format!(
                                    "X {:+.2}  Y {:+.2}  Z {:+.2} g",
                                    -st.ay_g, st.az_g, -st.ax_g
                                ),
                            );
                            kv_mono(
                                ui,
                                "Bias",
                                &format!(
                                    "{:+.2}  {:+.2}  {:+.2} °/s",
                                    status.gyro_bias[0], status.gyro_bias[1], status.gyro_bias[2]
                                ),
                            )
                            .on_hover_text(t(
                                l,
                                "Mantén Minus + Plus para recalibrar el giroscopio",
                                "Hold Minus + Plus to recalibrate the gyroscope",
                            ));
                        },
                    );
                },
            );
        });
    }

    fn ui_paddles(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let btns = status.telemetry.btns;
        let l = self.cfg.lang;
        card(
            ui,
            t(l, "Botones traseros independientes (PL / PR)", "Independent Back Paddles (PL / PR)"),
            None,
            0.0,
            |ui| {
                ui.label(
                    RichText::new(t(
                        l,
                        "Los botones traseros PL y PR llegan como bits independientes por la conexión 2.4 GHz. Pulsa cada uno para ver qué indicador se ilumina y elige su acción:",
                        "The PL and PR back paddles arrive as independent bits over the 2.4 GHz link. Press each paddle to see which indicator lights up, then choose its action:",
                    ))
                    .size(12.5)
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(14.0);

                paddle_row(
                    ui,
                    "PL",
                    t(l, "Trasero izquierdo (PL)", "Left back paddle (PL)"),
                    "bit 26 · 0x04000000",
                    (btns & BTN_P1) != 0,
                    "p1",
                    &mut self.cfg.paddle_p1,
                    l,
                );
                ui.add_space(8.0);
                paddle_row(
                    ui,
                    "PR",
                    t(l, "Trasero derecho (PR)", "Right back paddle (PR)"),
                    "bit 25 · 0x02000000",
                    (btns & BTN_P2) != 0,
                    "p2",
                    &mut self.cfg.paddle_p2,
                    l,
                );

                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    field_label(ui, t(l, "Máscara de botones actual", "Current raw button mask"));
                    ui.label(
                        RichText::new(format!("0x{:08X}", btns))
                            .monospace()
                            .size(12.5)
                            .color(ACCENT_CYAN),
                    );
                });
                ui.add_space(6.0);
                bit_strip(ui, btns);

                ui.add_space(16.0);
                egui::Frame::default()
                    .fill(lerp_color(BG_CARD, ACCENT_AMBER, 0.07))
                    .corner_radius(CornerRadius::same(10))
                    .inner_margin(egui::Margin::symmetric(14, 12))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            RichText::new(t(
                                l,
                                "¿Y los botones L4 y R4?",
                                "What about L4 and R4?",
                            ))
                            .strong()
                            .size(12.5)
                            .color(ACCENT_AMBER),
                        );
                        ui.add_space(2.0);
                        ui.label(
                            RichText::new(t(
                                l,
                                "Por la conexión 2.4 GHz, el dongle solo envía PL y PR como bits independientes; L4 y R4 sin asignar no generan un bit propio. Para usarlos, asígnales un botón estándar desde el propio mando o con 8BitDo Ultimate Software y funcionarán en cualquier juego.",
                                "Over the 2.4 GHz link, the dongle only sends PL and PR as independent bits; unmapped L4 and R4 do not produce a bit of their own. To use them, map them to a standard button on the controller itself or with 8BitDo Ultimate Software and they will work in any game.",
                            ))
                            .size(12.0)
                            .color(TEXT_SECONDARY),
                        );
                    });
            },
        );
    }

    fn ui_advanced(&mut self, ui: &mut egui::Ui, status: &BridgeStatus) {
        let l = self.cfg.lang;

        // ── Window / tray behavior ──
        card(
            ui,
            t(l, "Ventana y bandeja del sistema", "Window & System Tray"),
            None,
            0.0,
            |ui| {
                setting_row(ui, t(l, "Al pulsar X (cerrar)", "When clicking X (close)"), |ui| {
                    segmented(
                        ui,
                        &mut self.cfg.close_action,
                        &[
                            (CloseAction::Ask, t(l, "Preguntar siempre", "Always ask")),
                            (CloseAction::MinimizeToTray, t(l, "Minimizar a la bandeja", "Minimize to Tray")),
                            (CloseAction::Exit, t(l, "Cerrar completamente", "Exit completely")),
                        ],
                        &[],
                    );
                });
                let hint = match self.cfg.close_action {
                    CloseAction::Ask => t(
                        l,
                        "Se mostrará un diálogo cada vez que cierres la ventana.",
                        "A dialog appears every time you close the window.",
                    ),
                    CloseAction::MinimizeToTray => t(
                        l,
                        "La ventana se oculta en la bandeja del sistema y el puente sigue funcionando.",
                        "The window hides to the system tray and the bridge keeps running.",
                    ),
                    CloseAction::Exit => t(
                        l,
                        "La app se cierra y devuelve el dongle a XInput (~6 s): el mando se apaga un momento, solo tienes que volver a encenderlo.",
                        "The app exits and returns the dongle to stock XInput (~6 s): the controller turns off briefly — just turn it back on.",
                    ),
                };
                ui.label(RichText::new(hint).size(11.5).color(TEXT_MUTED));
                ui.add_space(10.0);
                toggle(
                    ui,
                    &mut self.cfg.auto_start,
                    t(
                        l,
                        "Iniciar el puente automáticamente al abrir la app",
                        "Start the bridge automatically when the app opens",
                    ),
                );
                ui.add_space(6.0);
                toggle(
                    ui,
                    &mut self.cfg.restore_xinput_on_exit,
                    t(
                        l,
                        "Restaurar XInput de fábrica al detener o salir (~6 s; luego vuelve a encender el mando)",
                        "Restore stock XInput on Stop or Exit (~6 s; then turn the controller back on)",
                    ),
                );
            },
        );

        ui.add_space(12.0);

        // ── Direct PC gyro ──
        let gyro_meta = if self.cfg.gyro_pc_mode == GyroPcMode::Off {
            (t(l, "desactivado", "off"), TEXT_MUTED)
        } else if status.gyro_pc_active {
            (t(l, "activo", "active"), ACCENT_GREEN)
        } else {
            (t(l, "en espera", "standby"), ACCENT_AMBER)
        };
        card(
            ui,
            t(
                l,
                "Giroscopio directo en PC (opcional, fuera de Steam Input)",
                "Direct PC Gyro Aiming (optional, outside Steam Input)",
            ),
            Some(gyro_meta),
            0.0,
            |ui| {
                ui.label(
                    RichText::new(t(
                        l,
                        "En modo DualShock 4, déjalo en «Desactivado» para que Steam Input gestione el giroscopio de forma nativa. Usa Gyro → Mouse o Gyro → Stick derecho solo en juegos fuera de Steam o en modo Xbox 360.",
                        "In DualShock 4 mode, leave this Off so Steam Input handles the gyro natively. Use Gyro → Mouse or Gyro → Right Stick only for non-Steam games or in Xbox 360 mode.",
                    ))
                    .size(12.0)
                    .color(TEXT_SECONDARY),
                );
                ui.add_space(10.0);

                setting_row(ui, t(l, "Modo", "Mode"), |ui| {
                    segmented(
                        ui,
                        &mut self.cfg.gyro_pc_mode,
                        &[
                            (GyroPcMode::Off, t(l, "Desactivado (recomendado en DS4)", "Off (recommended for DS4)")),
                            (GyroPcMode::Mouse, "Gyro → Mouse"),
                            (GyroPcMode::RightStick, t(l, "Gyro → Stick derecho", "Gyro → Right Stick")),
                        ],
                        &[],
                    );
                });

                if self.cfg.gyro_pc_mode != GyroPcMode::Off {
                    setting_row(ui, t(l, "Activar con", "Activate with"), |ui| {
                        segmented(
                            ui,
                            &mut self.cfg.gyro_trigger,
                            &[
                                (GyroTrigger::LeftTrigger, t(l, "Mantener L2", "Hold L2")),
                                (GyroTrigger::RightTrigger, t(l, "Mantener R2", "Hold R2")),
                                (GyroTrigger::LeftBumper, t(l, "Mantener L1", "Hold L1")),
                                (GyroTrigger::RightBumper, t(l, "Mantener R1", "Hold R1")),
                                (GyroTrigger::AlwaysOn, t(l, "Siempre activo", "Always on")),
                                (GyroTrigger::ToggleCapture, t(l, "Alternar (Star)", "Toggle (Star)")),
                            ],
                            &[],
                        );
                    });
                    ui.spacing_mut().slider_width = 260.0;
                    setting_row(ui, t(l, "Sensibilidad", "Sensitivity"), |ui| {
                        ui.add(egui::Slider::new(&mut self.cfg.gyro_sens, 0.2..=4.0));
                    });
                    setting_row(ui, t(l, "Mezcla de roll en eje X", "Roll mix on X axis"), |ui| {
                        ui.add(egui::Slider::new(&mut self.cfg.gyro_roll_mix, 0.0..=1.0));
                    });
                    setting_row(ui, t(l, "Zona muerta (°/s)", "Deadzone (°/s)"), |ui| {
                        ui.add(egui::Slider::new(&mut self.cfg.gyro_deadzone, 0.0..=5.0));
                    });
                    setting_row(ui, t(l, "Inversión", "Inversion"), |ui| {
                        toggle(ui, &mut self.cfg.invert_x, t(l, "Invertir eje X", "Invert X axis"));
                        ui.add_space(14.0);
                        toggle(ui, &mut self.cfg.invert_y, t(l, "Invertir eje Y", "Invert Y axis"));
                    });
                }
            },
        );

        ui.add_space(12.0);

        // ── DSU server + quick guide ──
        card(
            ui,
            t(l, "Servidor DSU / Cemuhook y guía rápida", "DSU / Cemuhook Server & Quick Guide"),
            None,
            0.0,
            |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    toggle(
                        ui,
                        &mut self.cfg.dsu_enabled,
                        t(l, "Activar servidor DSU (UDP, solo local)", "Enable DSU server (UDP, local only)"),
                    );
                    ui.add_space(18.0);
                    field_label(ui, t(l, "Puerto", "Port"));
                    ui.add(egui::DragValue::new(&mut self.cfg.dsu_port).range(1024..=65535));
                });
                ui.add_space(12.0);
                hairline(ui);
                ui.add_space(10.0);

                let addr = format!("127.0.0.1 : {}", self.cfg.dsu_port);
                let rows: [(&str, String); 5] = [
                    (
                        "Steam",
                        t(
                            l,
                            "Parámetros → Mando → Activar Steam Input para mandos de PlayStation.",
                            "Settings → Controller → Enable Steam Input for PlayStation controllers.",
                        )
                        .to_string(),
                    ),
                    (
                        "Dolphin",
                        format!(
                            "{} → {addr}",
                            t(l, "Mandos → Fuentes de entrada alternativas", "Controllers → Alternate Input Sources")
                        ),
                    ),
                    (
                        "Cemu",
                        format!(
                            "{} → {addr}",
                            t(
                                l,
                                "Opciones → Entrada → API: DSUController o Motion Source",
                                "Options → Input Settings → API: DSUController or Motion Source"
                            )
                        ),
                    ),
                    (
                        "Ryujinx / Yuzu / Eden",
                        format!(
                            "{} → {addr}",
                            t(l, "Controles → Movimiento → CemuHookUDP", "Controls → Motion → Use CemuHookUDP")
                        ),
                    ),
                    (
                        t(l, "XInput de fábrica", "Stock XInput"),
                        t(
                            l,
                            "Al pulsar Detener o salir, el dongle vuelve solo a XInput (~6 s). El mando se apagará un momento: vuelve a encenderlo y funcionará con normalidad.",
                            "Clicking Stop or exiting returns the dongle to stock XInput automatically (~6 s). The controller turns off briefly — just turn it back on and it works as usual.",
                        )
                        .to_string(),
                    ),
                ];
                for (name, desc) in rows.iter() {
                    ui.horizontal_top(|ui| {
                        ui.allocate_ui_with_layout(
                            Vec2::new(150.0, 18.0),
                            Layout::left_to_right(Align::Center),
                            |ui| {
                                ui.label(RichText::new(*name).size(12.5).strong().color(TEXT_PRIMARY));
                            },
                        );
                        ui.add(
                            egui::Label::new(RichText::new(desc).size(12.0).color(TEXT_SECONDARY))
                                .wrap(),
                        );
                    });
                }
                ui.add_space(12.0);
                hairline(ui);
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 10.0;
                    let (st_txt, st_col) = if self.vigem_installed {
                        (t(l, "ViGEmBus instalado · Listo para Steam", "ViGEmBus installed · Ready for Steam"), ACCENT_GREEN)
                    } else {
                        (t(l, "ViGEmBus no instalado", "ViGEmBus not installed"), ACCENT_AMBER)
                    };
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                    ui.painter().circle_filled(r.center(), 3.2, st_col);
                    ui.label(RichText::new(st_txt).size(12.0).color(TEXT_SECONDARY));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if ghost_button(
                            ui,
                            t(l, "Asistente de Steam / ViGEmBus", "Steam / ViGEmBus Setup"),
                            true,
                        )
                        .clicked()
                        {
                            self.refresh_setup_probes();
                            self.show_setup_modal = true;
                        }
                    });
                });
            },
        );

        ui.add_space(12.0);
        self.ui_about(ui);
        ui.add_space(4.0);
    }

    /// Slim "About" card: version, license, repository link and the AI-assisted /
    /// unofficial-project disclaimers.
    fn ui_about(&self, ui: &mut egui::Ui) {
        let l = self.cfg.lang;
        let version = format!("v{}", env!("CARGO_PKG_VERSION"));
        card(
            ui,
            t(l, "Acerca de", "About"),
            Some((&version, ACCENT_VIOLET)),
            0.0,
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    ui.label(RichText::new("Pro 3 Gyro Bridge").size(12.5).strong().color(TEXT_PRIMARY));
                    ui.label(RichText::new(format!("{version} ·")).size(12.0).color(TEXT_MUTED));
                    ui.label(RichText::new(t(l, "Licencia MIT ·", "MIT License ·")).size(12.0).color(TEXT_MUTED));
                    ui.hyperlink_to(
                        RichText::new("github.com/NicolasBeatum/pro3-24g-gyro-bridge").size(12.0),
                        REPO_URL,
                    )
                    .on_hover_text(REPO_URL);
                });
                ui.add_space(8.0);
                hairline(ui);
                ui.add_space(8.0);

                let note = |ui: &mut egui::Ui, title: &str, body: &str| {
                    ui.label(RichText::new(title).size(11.5).strong().color(TEXT_SECONDARY));
                    ui.add(egui::Label::new(RichText::new(body).size(11.5).color(TEXT_MUTED)).wrap());
                };
                ui.spacing_mut().item_spacing.y = 3.0;
                note(
                    ui,
                    t(l, "Desarrollo asistido por IA", "AI-assisted development"),
                    t(
                        l,
                        "Este proyecto se desarrolló con ayuda de herramientas de inteligencia artificial (Gemini Neon y Claude Opus 5.5) y su autor lo revisó y probó con hardware real. Aun así, puede contener errores: úsalo bajo tu propia responsabilidad.",
                        "This project was built with the help of AI coding tools (Gemini Neon and Claude Opus 5.5), then reviewed and tested by its author on real hardware. It may still contain bugs: use it at your own risk.",
                    ),
                );
                ui.add_space(6.0);
                note(
                    ui,
                    t(l, "Proyecto no oficial", "Unofficial project"),
                    t(
                        l,
                        "Proyecto comunitario independiente de interoperabilidad. No está afiliado, respaldado ni patrocinado por 8BitDo, Sony ni Microsoft; todas las marcas pertenecen a sus respectivos dueños. No modifica ningún firmware: todo ocurre en RAM y es reversible. Se ofrece «tal cual», sin garantías, bajo licencia MIT.",
                        "Independent community interoperability project. Not affiliated with, endorsed by, or sponsored by 8BitDo, Sony, or Microsoft; all trademarks belong to their respective owners. No firmware is ever modified: everything is RAM-only and reversible. Provided \"as is\", without warranty, under the MIT License.",
                    ),
                );
            },
        );
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Controller visualizer
// ═══════════════════════════════════════════════════════════════════════════

/// Renders the embedded 8BitDo Pro 3 SVG (viewBox 640x430) with live, soft-glowing overlays
/// anchored on the exact button coordinates. Designed to read cleanly over a line-art outline.
fn draw_pro3_controller(
    ui: &mut egui::Ui,
    tex: Option<&egui::TextureHandle>,
    st: &TelemetryState,
    rumble_l: u8,
    rumble_r: u8,
    max_h: f32,
) {
    let avail_w = ui.available_width();
    // Room for the L2/R2 rails; the SVG's own top margin (bumpers start at y=27) does the rest.
    let top_pad = 10.0;
    let w = avail_w
        .min(((max_h - top_pad - 4.0) * (640.0 / 430.0)).max(300.0))
        .min(560.0);
    let svg_h = w * (430.0 / 640.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(avail_w, svg_h + top_pad), Sense::hover());
    let p = ui.painter();

    let scale = w / 640.0;
    let ox = rect.center().x - 320.0 * scale;
    let oy = rect.top() + top_pad;
    let pt = |x: f32, y: f32| Pos2::new(ox + x * scale, oy + y * scale);
    let sc = |v: f32| v * scale;
    let btns = st.btns;
    let on = |mask: u32| (btns & mask) != 0;

    // 1) L2 / R2 analog trigger bars — thin rails right above the shoulders.
    trigger_rail(
        p,
        ox + 70.0 * scale,
        ox + 206.0 * scale,
        rect.top() + 4.0,
        "L2",
        st.l2,
        on(BTN_L2_DIG),
        false,
    );
    trigger_rail(
        p,
        ox + 434.0 * scale,
        ox + 570.0 * scale,
        rect.top() + 4.0,
        "R2",
        st.r2,
        on(BTN_R2_DIG),
        true,
    );

    // 2) Rumble halos sit *behind* the outline so the line art stays crisp.
    for (val, cx) in [(rumble_l, 118.0), (rumble_r, 522.0)] {
        if val > 0 {
            let k = val as f32 / 255.0;
            let c = pt(cx, 334.0);
            for (rr, a) in [(78.0, 10.0), (60.0, 20.0), (44.0, 34.0), (28.0, 48.0)] {
                p.circle_filled(c, sc(rr), with_alpha(ACCENT_AMBER, (a * k) as u8));
            }
        }
    }

    // 3) Controller artwork.
    let img_rect = Rect::from_min_size(Pos2::new(ox, oy), Vec2::new(640.0 * scale, 430.0 * scale));
    if let Some(tx) = tex {
        p.image(
            tx.id(),
            img_rect,
            Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    // 4) L1 / R1 bumpers: curved bands concentric with the upper lobes (centre (162,148),
    //    band radius 116.25, 7.5 thick), sweeping from -152° over the top to x=206.
    let bumper = |mirror: bool| {
        let (lcx, lcy, br) = (162.0f32, 148.0f32, 116.25f32);
        let a0 = (-152.0f32).to_radians();
        let mut pts: Vec<Pos2> = (0..=28)
            .map(|i| {
                let a = a0 + (-FRAC_PI_2 - a0) * (i as f32 / 28.0);
                (lcx + br * a.cos(), lcy + br * a.sin())
            })
            .chain(std::iter::once((205.0, lcy - br)))
            .map(|(x, y)| if mirror { pt(640.0 - x, y) } else { pt(x, y) })
            .collect();
        pts.dedup();
        let band = sc(7.5);
        for (extra, a) in [(14.0, 8u8), (8.0, 18), (3.5, 40)] {
            let w = band + sc(extra);
            p.add(Shape::line(pts.clone(), Stroke::new(w, with_alpha(ACCENT_CYAN, a))));
            p.circle_filled(pts[0], w * 0.5, with_alpha(ACCENT_CYAN, a));
        }
        p.circle_filled(pts[0], band * 0.5, with_alpha(ACCENT_CYAN, 210));
        p.add(Shape::line(pts, Stroke::new(band, with_alpha(ACCENT_CYAN, 210))));
    };
    if on(BTN_L1) {
        bumper(false);
    }
    if on(BTN_R1) {
        bumper(true);
    }

    // 5) D-Pad arms around (162,121): arms ±44 long, 31 wide.
    let dpad = [
        (BTN_DUP, (146.5, 77.0, 177.5, 108.5)),
        (BTN_DDOWN, (146.5, 133.5, 177.5, 165.0)),
        (BTN_DLEFT, (118.0, 105.5, 149.5, 136.5)),
        (BTN_DRIGHT, (174.5, 105.5, 206.0, 136.5)),
    ];
    for (mask, (x0, y0, x1, y1)) in dpad {
        if on(mask) {
            glow_rect(p, Rect::from_min_max(pt(x0, y0), pt(x1, y1)), sc(5.0), ACCENT_CYAN, scale);
        }
    }

    // 6) Face buttons — Xbox-like hue mapping on the refined palette.
    let face = [
        (BTN_NORTH, (478.0, 78.0), ACCENT_AMBER),
        (BTN_WEST, (434.0, 122.0), ACCENT_CYAN),
        (BTN_EAST, (522.0, 122.0), ACCENT_CORAL),
        (BTN_SOUTH, (478.0, 166.0), ACCENT_GREEN),
    ];
    for (mask, (x, y), col) in face {
        if on(mask) {
            glow_circle(p, pt(x, y), sc(18.0), col);
        }
    }

    // 7) -select / +start / Star / Home.
    if on(BTN_MINUS) {
        glow_rect(p, Rect::from_min_max(pt(273.0, 115.5), pt(313.0, 128.5)), sc(6.5), ACCENT_CYAN, scale);
    }
    if on(BTN_PLUS) {
        glow_rect(p, Rect::from_min_max(pt(327.0, 115.5), pt(367.0, 128.5)), sc(6.5), ACCENT_CYAN, scale);
    }
    if on(BTN_CAPTURE) {
        glow_circle(p, pt(162.0, 216.0), sc(11.0), ACCENT_AMBER);
    }
    if on(BTN_HOME) {
        glow_circle(p, pt(478.0, 216.0), sc(11.0), ACCENT_VIOLET);
    }

    // 8) Analog sticks — dark well, faint crosshair, crisp moving ring + dot.
    let stick = |cx: f32, cy: f32, x: u8, y: u8, clicked: bool| {
        let c = pt(cx, cy);
        let mut nx = (x as f32 - 127.5) / 127.5;
        let mut ny = (y as f32 - 127.5) / 127.5;
        let mag = (nx * nx + ny * ny).sqrt();
        if mag > 1.0 {
            nx /= mag;
            ny /= mag;
        }
        let moved = mag > 0.06;
        let well_r = sc(31.0);
        let col = if clicked {
            ACCENT_GREEN
        } else if moved {
            ACCENT_CYAN
        } else {
            Color32::from_rgb(118, 128, 150)
        };

        if clicked {
            for (e, a) in [(9.0, 14u8), (5.0, 30), (2.0, 60)] {
                p.circle_filled(c, well_r + sc(e), with_alpha(ACCENT_GREEN, a));
            }
        }
        p.circle_filled(c, well_r, Color32::from_rgba_unmultiplied(10, 12, 18, 205));
        p.circle_stroke(
            c,
            well_r,
            Stroke::new(1.0, if clicked { ACCENT_GREEN } else { with_alpha(TEXT_SECONDARY, 70) }),
        );
        let ch = with_alpha(TEXT_SECONDARY, 26);
        p.line_segment([Pos2::new(c.x - well_r + sc(6.0), c.y), Pos2::new(c.x + well_r - sc(6.0), c.y)], Stroke::new(1.0, ch));
        p.line_segment([Pos2::new(c.x, c.y - well_r + sc(6.0)), Pos2::new(c.x, c.y + well_r - sc(6.0))], Stroke::new(1.0, ch));

        let travel = well_r - sc(10.0);
        let knob = Pos2::new(c.x + nx * travel, c.y + ny * travel);
        if moved {
            p.line_segment([c, knob], Stroke::new(1.2, with_alpha(col, 110)));
            p.circle_filled(knob, sc(14.0), with_alpha(col, 34));
        }
        p.circle_stroke(knob, sc(8.5), Stroke::new(1.8, col));
        p.circle_filled(knob, sc(3.2).max(2.0), col);
    };
    stick(240.0, 206.0, st.lx, st.ly, on(BTN_L3));
    stick(400.0, 206.0, st.rx, st.ry, on(BTN_R3));

    // 9) PL / PR rear paddle pills in the open space between the grips.
    let pill = |cx: f32, cy: f32, text: &str, active: bool| {
        let size = Vec2::new(sc(52.0).max(38.0), sc(22.0).max(16.0));
        let r = Rect::from_center_size(pt(cx, cy), size);
        let cr = CornerRadius::same((size.y * 0.5) as u8);
        if active {
            for (e, a) in [(6.0, 16u8), (3.0, 36)] {
                p.rect_filled(r.expand(e), CornerRadius::same((size.y * 0.5 + e) as u8), with_alpha(ACCENT_GREEN, a));
            }
            p.rect_filled(r, cr, ACCENT_GREEN);
        } else {
            p.rect_filled(r, cr, Color32::from_rgba_unmultiplied(16, 19, 28, 225));
            p.rect_stroke(r, cr, Stroke::new(1.0, STROKE_STRONG), StrokeKind::Inside);
        }
        p.text(
            r.center(),
            Align2::CENTER_CENTER,
            text,
            FontId::proportional((sc(12.5)).max(10.0)),
            if active { BG_CANVAS } else { TEXT_SECONDARY },
        );
    };
    pill(276.0, 326.0, "PL", on(BTN_P1));
    pill(364.0, 326.0, "PR", on(BTN_P2));
}

/// Thin analog trigger rail with label + tabular value, mirrored for the right trigger.
#[allow(clippy::too_many_arguments)]
fn trigger_rail(p: &Painter, x0: f32, x1: f32, top: f32, label: &str, value: u8, digital: bool, mirror: bool) {
    let frac = value as f32 / 255.0;
    let label_col = if digital || value > 8 { TEXT_PRIMARY } else { TEXT_MUTED };
    let (la, va, lx, vx) = if mirror {
        (Align2::RIGHT_CENTER, Align2::LEFT_CENTER, x1, x0)
    } else {
        (Align2::LEFT_CENTER, Align2::RIGHT_CENTER, x0, x1)
    };
    p.text(Pos2::new(lx, top + 6.0), la, label, FontId::proportional(11.0), label_col);
    p.text(
        Pos2::new(vx, top + 6.0),
        va,
        format!("{value:>3}"),
        FontId::monospace(10.5),
        if value > 8 { ACCENT_CYAN } else { TEXT_MUTED },
    );

    let track = Rect::from_min_max(Pos2::new(x0, top + 15.0), Pos2::new(x1, top + 18.0));
    p.rect_filled(track, CornerRadius::same(2), BG_RAISED);
    if frac > 0.01 {
        let fill = if mirror {
            Rect::from_min_max(Pos2::new(x1 - track.width() * frac, track.top()), track.max)
        } else {
            Rect::from_min_max(track.min, Pos2::new(x0 + track.width() * frac, track.bottom()))
        };
        p.rect_filled(fill.expand(2.5), CornerRadius::same(4), with_alpha(ACCENT_CYAN, 30));
        p.rect_filled(fill, CornerRadius::same(2), ACCENT_CYAN);
        let tip = if mirror { fill.left_center() } else { fill.right_center() };
        p.circle_filled(tip, 3.0, Color32::WHITE);
    }
}

fn glow_circle(p: &Painter, c: Pos2, r: f32, col: Color32) {
    p.circle_filled(c, r * 1.9, with_alpha(col, 7));
    p.circle_filled(c, r * 1.5, with_alpha(col, 16));
    p.circle_filled(c, r * 1.2, with_alpha(col, 34));
    p.circle_filled(c, r, with_alpha(col, 140));
    p.circle_stroke(c, r, Stroke::new(1.6, col));
    p.circle_filled(c, r * 0.32, with_alpha(Color32::WHITE, 110));
}

fn glow_rect(p: &Painter, rect: Rect, radius: f32, col: Color32, scale: f32) {
    for (e, a) in [(9.0, 7u8), (5.5, 16), (2.5, 36)] {
        let e = e * scale.max(0.6);
        p.rect_filled(rect.expand(e), CornerRadius::same((radius + e) as u8), with_alpha(col, a));
    }
    let cr = CornerRadius::same(radius as u8);
    p.rect_filled(rect, cr, with_alpha(col, 150));
    p.rect_stroke(rect, cr, Stroke::new(1.4, col), StrokeKind::Inside);
}

// ═══════════════════════════════════════════════════════════════════════════
// Gyroscope horizon gauge
// ═══════════════════════════════════════════════════════════════════════════

/// Artificial horizon driven by the 3-axis accelerometer, with roll pointer, pitch ladder,
/// degree ticks and a yaw-rate arc on the outer ring.
fn draw_horizon_gauge(ui: &mut egui::Ui, ax: f32, ay: f32, az: f32, yaw_dps: f32, size: f32) {
    ui.vertical_centered(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
        let p = ui.painter();
        let c = rect.center();
        let r = size * 0.5 - 5.0; // outer ring (yaw arc / ticks)
        let face_r = r - 10.0; // attitude face

        // Bezel + face
        p.circle_filled(c, r, BG_INSET);
        p.circle_stroke(c, r, Stroke::new(1.0, STROKE));
        p.circle_filled(c, face_r, Color32::from_rgb(16, 20, 30));

        let roll = ax.atan2(ay.max(0.05)).clamp(-1.1, 1.1);
        let pitch = az.atan2((ax * ax + ay * ay).sqrt().max(0.05)).clamp(-1.0, 1.0);
        let k = face_r * 0.65;
        let pitch_px = pitch * k;
        let d = Vec2::new(roll.cos(), roll.sin()); // along horizon
        let n = Vec2::new(-roll.sin(), roll.cos()); // toward ground
        let mid = c + n * pitch_px;

        // Ground fill: circular segment below the horizon (convex → sorted circle points).
        let mut pts: Vec<Pos2> = (0..96)
            .map(|i| {
                let a = i as f32 / 96.0 * TAU;
                c + Vec2::new(a.cos(), a.sin()) * face_r
            })
            .filter(|q| (*q - mid).dot(n) > 0.0)
            .collect();
        let m = mid - c;
        let b = m.dot(d);
        let disc = b * b - (m.length_sq() - face_r * face_r);
        let mut ends = None;
        if disc > 0.0 {
            let s = disc.sqrt();
            let e1 = mid + d * (-b - s);
            let e2 = mid + d * (-b + s);
            pts.push(e1);
            pts.push(e2);
            ends = Some((e1, e2));
        }
        pts.sort_by(|a, bb| {
            let aa = (a.y - c.y).atan2(a.x - c.x);
            let ab = (bb.y - c.y).atan2(bb.x - c.x);
            aa.partial_cmp(&ab).unwrap_or(std::cmp::Ordering::Equal)
        });
        if pts.len() >= 3 {
            p.add(Shape::convex_polygon(
                pts,
                lerp_color(Color32::from_rgb(16, 20, 30), ACCENT_CYAN, 0.13),
                Stroke::NONE,
            ));
        }

        // Inner concentric guide ring
        p.circle_stroke(c, face_r * 0.55, Stroke::new(1.0, with_alpha(TEXT_SECONDARY, 22)));

        // Pitch ladder (±10°, ±20°)
        let step = 10f32.to_radians() * k;
        for i in [-2i32, -1, 1, 2] {
            let o = mid + n * (i as f32 * step);
            if (o - c).length() < face_r - 8.0 {
                let half = if i.abs() == 1 { face_r * 0.2 } else { face_r * 0.12 };
                p.line_segment([o - d * half, o + d * half], Stroke::new(1.0, with_alpha(TEXT_PRIMARY, 46)));
            }
        }

        // Horizon line
        if let Some((e1, e2)) = ends {
            p.line_segment([e1, e2], Stroke::new(1.8, ACCENT_CYAN));
        }
        p.circle_stroke(c, face_r, Stroke::new(1.0, STROKE_STRONG));

        // Degree ticks every 15° (major every 45°)
        for deg in (0..360).step_by(15) {
            let a = (deg as f32).to_radians() - FRAC_PI_2;
            let u = Vec2::new(a.cos(), a.sin());
            let major = deg % 45 == 0;
            let len = if major { 5.0 } else { 2.5 };
            let col = if major { Color32::from_rgb(92, 102, 126) } else { Color32::from_rgb(52, 60, 80) };
            p.line_segment([c + u * (r - 2.0), c + u * (r - 2.0 - len)], Stroke::new(1.0, col));
        }

        // Roll pointer: small triangle riding the face edge
        let ra = roll - FRAC_PI_2;
        let u = Vec2::new(ra.cos(), ra.sin());
        let perp = Vec2::new(-u.y, u.x);
        let tip = c + u * (face_r - 1.0);
        let base = c + u * (face_r - 8.0);
        p.add(Shape::convex_polygon(
            vec![tip, base + perp * 4.0, base - perp * 4.0],
            ACCENT_CYAN,
            Stroke::NONE,
        ));

        // Yaw-rate arc on the outer ring
        let norm = (yaw_dps / 240.0).clamp(-1.0, 1.0);
        let start = -FRAC_PI_2;
        let sweep = -norm * 1.3;
        if sweep.abs() > 0.02 {
            let segs = 32;
            let arc: Vec<Pos2> = (0..=segs)
                .map(|i| {
                    let a = start + sweep * (i as f32 / segs as f32);
                    c + Vec2::new(a.cos(), a.sin()) * (r - 0.5)
                })
                .collect();
            p.add(Shape::line(arc, Stroke::new(2.4, ACCENT_VIOLET)));
        }
        let ya = start + sweep;
        p.circle_filled(c + Vec2::new(ya.cos(), ya.sin()) * (r - 0.5), 3.4, ACCENT_VIOLET);

        // Fixed aircraft reticle
        let wing = Stroke::new(1.8, ACCENT_AMBER);
        p.line_segment([Pos2::new(c.x - 18.0, c.y), Pos2::new(c.x - 7.0, c.y)], wing);
        p.line_segment([Pos2::new(c.x + 7.0, c.y), Pos2::new(c.x + 18.0, c.y)], wing);
        p.circle_stroke(c, 4.0, Stroke::new(1.5, ACCENT_AMBER));
    });
}

/// Slim signed bar: label left, centred zero, crisp right-aligned tabular value.
fn axis_bar(ui: &mut egui::Ui, label: &str, value: f32, max_abs: f32, color: Color32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 18.0), Sense::hover());
    let p = ui.painter();
    let label_w = 40.0;
    let value_w = 78.0;
    p.text(
        Pos2::new(rect.left(), rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        TEXT_SECONDARY,
    );
    p.text(
        Pos2::new(rect.right(), rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{:+7.1} °/s", value),
        FontId::monospace(11.0),
        TEXT_PRIMARY,
    );

    let x0 = rect.left() + label_w;
    let x1 = (rect.right() - value_w).max(x0 + 20.0);
    let cy = rect.center().y;
    let track = Rect::from_min_max(Pos2::new(x0, cy - 1.5), Pos2::new(x1, cy + 1.5));
    p.rect_filled(track, CornerRadius::same(2), BG_RAISED);
    let mid_x = track.center().x;
    p.line_segment(
        [Pos2::new(mid_x, cy - 5.0), Pos2::new(mid_x, cy + 5.0)],
        Stroke::new(1.0, Color32::from_rgb(70, 80, 104)),
    );
    let norm = (value / max_abs).clamp(-1.0, 1.0);
    let end_x = mid_x + norm * track.width() * 0.5;
    if (end_x - mid_x).abs() > 0.5 {
        let fill = Rect::from_min_max(
            Pos2::new(mid_x.min(end_x), track.top()),
            Pos2::new(mid_x.max(end_x), track.bottom()),
        );
        p.rect_filled(fill.expand(2.0), CornerRadius::same(3), with_alpha(color, 34));
        p.rect_filled(fill, CornerRadius::same(2), color);
        p.circle_filled(Pos2::new(end_x, cy), 3.0, color);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Widgets & layout primitives
// ═══════════════════════════════════════════════════════════════════════════

fn with_alpha(c: Color32, a: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), a)
}

fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn ellipsize(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

/// Soft card: single 1px hairline stroke, generous padding, optional right-aligned meta.
fn card(
    ui: &mut egui::Ui,
    title: &str,
    meta: Option<(&str, Color32)>,
    min_h: f32,
    add_contents: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    egui::Frame::default()
        .fill(BG_CARD)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(CornerRadius::same(12))
        .inner_margin(egui::Margin::same(16))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            if min_h > 0.0 {
                ui.set_min_height(min_h - 34.0);
            }
            if !title.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
                    if let Some((m, col)) = meta {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.label(RichText::new(m).size(11.5).color(TEXT_SECONDARY));
                            let (r, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
                            ui.painter().circle_filled(r.center(), 3.0, col);
                        });
                    }
                });
                ui.add_space(10.0);
            }
            add_contents(ui);
        })
        .response
}

/// Sleek segmented control with an animated sliding selection thumb.
fn segmented<T: PartialEq + Copy>(
    ui: &mut egui::Ui,
    value: &mut T,
    options: &[(T, &str)],
    tips: &[&str],
) -> bool {
    let font = FontId::proportional(12.5);
    let pad_x = 12.0;
    let h = 28.0;
    let inset = 3.0;
    let widths: Vec<f32> = options
        .iter()
        .map(|(_, lbl)| {
            ui.painter()
                .layout_no_wrap(lbl.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x
                + pad_x * 2.0
        })
        .collect();
    let total: f32 = widths.iter().sum::<f32>() + inset * 2.0;
    let (rect, base) = ui.allocate_exact_size(Vec2::new(total, h), Sense::hover());

    let mut changed = false;
    let mut segs = Vec::with_capacity(options.len());
    let mut x = rect.left() + inset;
    for (i, ((val, _), w)) in options.iter().zip(widths.iter()).enumerate() {
        let seg = Rect::from_min_size(Pos2::new(x, rect.top() + inset), Vec2::new(*w, h - inset * 2.0));
        let mut r = ui
            .interact(seg, base.id.with(i), Sense::click())
            .on_hover_cursor(CursorIcon::PointingHand);
        if let Some(tip) = tips.get(i) {
            if !tip.is_empty() {
                r = r.on_hover_text(*tip);
            }
        }
        if r.clicked() && *value != *val {
            *value = *val;
            changed = true;
        }
        segs.push((seg, r.hovered()));
        x += w;
    }

    let sel = options.iter().position(|(v, _)| *v == *value).unwrap_or(0);
    let ctx = ui.ctx().clone();
    let tx = ctx.animate_value_with_time(base.id.with("thumb_x"), segs[sel].0.left(), 0.14);
    let tw = ctx.animate_value_with_time(base.id.with("thumb_w"), segs[sel].0.width(), 0.14);

    let p = ui.painter();
    p.rect_filled(rect, CornerRadius::same(9), BG_INSET);
    p.rect_stroke(rect, CornerRadius::same(9), Stroke::new(1.0, STROKE), StrokeKind::Inside);
    for (i, (seg, hovered)) in segs.iter().enumerate() {
        if *hovered && i != sel {
            p.rect_filled(*seg, CornerRadius::same(7), Color32::from_rgb(22, 27, 39));
        }
    }
    let thumb = Rect::from_min_size(Pos2::new(tx, rect.top() + inset), Vec2::new(tw, h - inset * 2.0));
    p.rect_filled(thumb, CornerRadius::same(7), BG_RAISED);
    p.rect_stroke(thumb, CornerRadius::same(7), Stroke::new(1.0, STROKE_STRONG), StrokeKind::Inside);

    for (i, ((_, lbl), (seg, hovered))) in options.iter().zip(segs.iter()).enumerate() {
        let col = if i == sel {
            TEXT_PRIMARY
        } else if *hovered {
            TEXT_SECONDARY
        } else {
            TEXT_MUTED
        };
        p.text(seg.center(), Align2::CENTER_CENTER, *lbl, font.clone(), col);
    }
    changed
}

/// iOS / Fluent style toggle switch with a clickable label.
fn toggle(ui: &mut egui::Ui, on: &mut bool, label: &str) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(32.0, 18.0), Sense::click());
        let lbl = ui.add(
            egui::Label::new(RichText::new(label).size(12.5).color(TEXT_PRIMARY)).sense(Sense::click()),
        );
        if resp.clicked() || lbl.clicked() {
            *on = !*on;
            changed = true;
        }
        let anim_id = resp.id;
        resp.on_hover_cursor(CursorIcon::PointingHand);
        lbl.on_hover_cursor(CursorIcon::PointingHand);

        let tt = ui.ctx().animate_bool(anim_id, *on);
        let p = ui.painter();
        let track = lerp_color(Color32::from_rgb(36, 43, 60), Color32::from_rgb(16, 150, 108), tt);
        p.rect_filled(rect, CornerRadius::same(9), track);
        if tt < 0.5 {
            p.rect_stroke(rect, CornerRadius::same(9), Stroke::new(1.0, STROKE_STRONG), StrokeKind::Inside);
        }
        let kx = egui::lerp((rect.left() + 9.0)..=(rect.right() - 9.0), tt);
        p.circle_filled(Pos2::new(kx, rect.center().y), 6.5, Color32::from_rgb(240, 244, 250));
    });
    changed
}

fn solid_button(ui: &mut egui::Ui, text: &str, fill: Color32, fg: Color32, stroke: Stroke) -> egui::Response {
    ui.add(
        egui::Button::new(RichText::new(text).size(13.0).strong().color(fg))
            .fill(fill)
            .stroke(stroke)
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(88.0, 32.0)),
    )
    .on_hover_cursor(CursorIcon::PointingHand)
}

fn ghost_button(ui: &mut egui::Ui, text: &str, enabled: bool) -> egui::Response {
    ui.add_enabled(
        enabled,
        egui::Button::new(RichText::new(text).size(12.5))
            .corner_radius(CornerRadius::same(8))
            .min_size(Vec2::new(0.0, 32.0)),
    )
}

/// Underline navigation tabs.
fn tab_bar(ui: &mut egui::Ui, current: &mut Tab, items: &[(Tab, &str)]) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let font = FontId::proportional(13.0);
        for (i, (tab, label)) in items.iter().enumerate() {
            let gw = ui
                .painter()
                .layout_no_wrap(label.to_string(), font.clone(), Color32::WHITE)
                .size()
                .x;
            if i == 0 {
                ui.add_space(-12.0);
            }
            let (rect, resp) = ui.allocate_exact_size(Vec2::new(gw + 24.0, 34.0), Sense::click());
            let resp = resp.on_hover_cursor(CursorIcon::PointingHand);
            if resp.clicked() {
                *current = *tab;
            }
            let selected = *current == *tab;
            let col = if selected {
                TEXT_PRIMARY
            } else if resp.hovered() {
                TEXT_SECONDARY
            } else {
                TEXT_MUTED
            };
            let p = ui.painter();
            p.text(Pos2::new(rect.center().x, rect.center().y - 1.0), Align2::CENTER_CENTER, *label, font.clone(), col);
            let ul = Rect::from_min_max(
                Pos2::new(rect.center().x - gw * 0.5, rect.bottom() - 2.0),
                Pos2::new(rect.center().x + gw * 0.5, rect.bottom()),
            );
            if selected {
                p.rect_filled(ul.expand2(Vec2::new(0.0, 2.0)), CornerRadius::same(2), with_alpha(ACCENT_CYAN, 30));
                p.rect_filled(ul, CornerRadius::same(1), ACCENT_CYAN);
            } else if resp.hovered() {
                p.rect_filled(ul, CornerRadius::same(1), STROKE_STRONG);
            }
        }
    });
}

fn strip_item(ui: &mut egui::Ui, label: &str, value: &str, value_col: Color32) -> egui::Response {
    ui.label(RichText::new(label).size(11.5).color(TEXT_MUTED));
    ui.label(RichText::new(value).size(11.5).color(value_col))
}

fn strip_sep(ui: &mut egui::Ui) {
    ui.add_space(4.0);
    let (r, _) = ui.allocate_exact_size(Vec2::new(3.0, 12.0), Sense::hover());
    ui.painter().circle_filled(r.center(), 1.4, STROKE_STRONG);
    ui.add_space(4.0);
}

fn battery_glyph(ui: &mut egui::Ui, pct: u8, known: bool) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(19.0, 10.0), Sense::hover());
    let p = ui.painter();
    let body = Rect::from_min_max(r.min, Pos2::new(r.right() - 2.0, r.bottom()));
    p.rect_stroke(body, CornerRadius::same(2), Stroke::new(1.0, Color32::from_rgb(92, 102, 126)), StrokeKind::Inside);
    p.rect_filled(
        Rect::from_min_max(Pos2::new(body.right(), r.center().y - 2.0), Pos2::new(r.right(), r.center().y + 2.0)),
        CornerRadius::same(1),
        Color32::from_rgb(92, 102, 126),
    );
    if known {
        let frac = (pct as f32 / 100.0).clamp(0.0, 1.0);
        let inner = body.shrink(2.0);
        let col = if pct < 25 { ACCENT_CORAL } else { ACCENT_GREEN };
        if frac > 0.0 {
            p.rect_filled(
                Rect::from_min_max(inner.min, Pos2::new(inner.left() + inner.width() * frac, inner.bottom())),
                CornerRadius::same(1),
                col,
            );
        }
    }
}

fn field_label(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).size(12.0).color(TEXT_MUTED));
}

fn hairline(ui: &mut egui::Ui) {
    let (r, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
    ui.painter().rect_filled(r, CornerRadius::ZERO, STROKE);
}

fn kv_mono(ui: &mut egui::Ui, key: &str, value: &str) -> egui::Response {
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(Vec2::new(40.0, 16.0), Layout::left_to_right(Align::Center), |ui| {
            ui.label(RichText::new(key).size(11.5).color(TEXT_MUTED));
        });
        ui.label(RichText::new(value).monospace().size(11.0).color(TEXT_SECONDARY));
    })
    .response
}

/// Label column + control, for settings forms.
fn setting_row(ui: &mut egui::Ui, label: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal(|ui| {
        ui.set_min_height(32.0);
        ui.allocate_ui_with_layout(Vec2::new(170.0, 28.0), Layout::left_to_right(Align::Center), |ui| {
            ui.set_min_width(170.0);
            ui.label(RichText::new(label).size(12.5).color(TEXT_SECONDARY));
        });
        add(ui);
    });
}

fn option_tile(ui: &mut egui::Ui, accent: Color32, title: &str, desc: &str, desc_col: Color32) {
    egui::Frame::default()
        .fill(BG_INSET)
        .stroke(Stroke::new(1.0, STROKE))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 10.0;
                let (r, _) = ui.allocate_exact_size(Vec2::new(8.0, 18.0), Sense::hover());
                ui.painter().circle_filled(r.center(), 3.5, accent);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 3.0;
                    ui.label(RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
                    ui.add(egui::Label::new(RichText::new(desc).size(12.0).color(desc_col)).wrap());
                });
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn paddle_row(
    ui: &mut egui::Ui,
    tag: &str,
    title: &str,
    caption: &str,
    pressed: bool,
    id: &str,
    action: &mut PaddleAction,
    lang: Language,
) {
    egui::Frame::default()
        .fill(if pressed { lerp_color(BG_INSET, ACCENT_GREEN, 0.08) } else { BG_INSET })
        .stroke(Stroke::new(1.0, if pressed { with_alpha(ACCENT_GREEN, 90) } else { STROKE }))
        .corner_radius(CornerRadius::same(10))
        .inner_margin(egui::Margin::symmetric(14, 10))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 12.0;
                // Live tag pill
                let (r, _) = ui.allocate_exact_size(Vec2::new(44.0, 26.0), Sense::hover());
                let p = ui.painter();
                let cr = CornerRadius::same(13);
                if pressed {
                    p.rect_filled(r.expand(3.0), CornerRadius::same(16), with_alpha(ACCENT_GREEN, 30));
                    p.rect_filled(r, cr, ACCENT_GREEN);
                } else {
                    p.rect_filled(r, cr, BG_CARD);
                    p.rect_stroke(r, cr, Stroke::new(1.0, STROKE_STRONG), StrokeKind::Inside);
                }
                p.text(
                    r.center(),
                    Align2::CENTER_CENTER,
                    tag,
                    FontId::proportional(12.5),
                    if pressed { BG_CANVAS } else { TEXT_SECONDARY },
                );

                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 1.0;
                    ui.label(RichText::new(title).size(13.0).strong().color(TEXT_PRIMARY));
                    ui.label(RichText::new(caption).monospace().size(10.5).color(TEXT_MUTED));
                });

                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    egui::ComboBox::from_id_salt(id)
                        .selected_text(action.label(lang))
                        .width(240.0)
                        .show_ui(ui, |ui| {
                            for &opt in PaddleAction::ALL {
                                ui.selectable_value(action, opt, opt.label(lang));
                            }
                        });
                    if pressed {
                        ui.label(
                            RichText::new(t(lang, "PULSADO", "PRESSED"))
                                .size(11.0)
                                .strong()
                                .color(ACCENT_GREEN),
                        );
                    }
                });
            });
        });
}

/// 32-cell live visualisation of the raw button bitmask (bit 31 → 0), PL/PR bits outlined.
fn bit_strip(ui: &mut egui::Ui, btns: u32) {
    let cell = Vec2::new(9.0, 16.0);
    let gap = 3.0;
    let group_gap = 7.0;
    let total_w = 32.0 * cell.x + 31.0 * gap + 3.0 * group_gap;
    let (rect, resp) = ui.allocate_exact_size(Vec2::new(total_w + 50.0, cell.y + 14.0), Sense::hover());
    let p = ui.painter();
    let mut hovered_bit = None;
    for i in 0..32u32 {
        let bit = 31 - i;
        let x = rect.left() + i as f32 * (cell.x + gap) + (i / 8) as f32 * group_gap;
        let r = Rect::from_min_size(Pos2::new(x, rect.top()), cell);
        let set = (btns >> bit) & 1 == 1;
        let paddle = bit == 25 || bit == 26;
        let fill = match (set, paddle) {
            (true, true) => ACCENT_GREEN,
            (true, false) => ACCENT_CYAN,
            _ => BG_RAISED,
        };
        if set {
            p.rect_filled(r.expand(2.0), CornerRadius::same(4), with_alpha(fill, 34));
        }
        p.rect_filled(r, CornerRadius::same(2), fill);
        if paddle && !set {
            p.rect_stroke(r, CornerRadius::same(2), Stroke::new(1.0, with_alpha(ACCENT_GREEN, 140)), StrokeKind::Inside);
        }
        if bit % 8 == 7 || bit == 0 {
            p.text(
                Pos2::new(r.center().x, r.bottom() + 7.0),
                Align2::CENTER_CENTER,
                bit.to_string(),
                FontId::monospace(9.0),
                TEXT_MUTED,
            );
        }
        if resp.hovered() {
            if let Some(pos) = resp.hover_pos() {
                if r.expand(1.5).contains(pos) {
                    hovered_bit = Some((bit, set));
                }
            }
        }
    }
    if let Some((bit, set)) = hovered_bit {
        resp.on_hover_text(format!("bit {bit} = {}", set as u8));
    }
}
