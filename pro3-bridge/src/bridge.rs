use crate::config::*;
use crate::dsu::DsuServer;
use crate::input_sim::{send_key_state, send_mouse_delta};
use crate::protocol::*;
use crate::vigem::VigemBridge;
use hidapi::HidApi;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct BridgeStatus {
    pub running: bool,
    pub state_text: String,
    pub connected: bool,
    pub rate_hz: f32,
    pub battery: u8,
    pub dsu_clients: usize,
    pub dsu_error: Option<String>,
    pub vigem_status: String,
    pub vigem_ok: bool,
    pub gyro_bias: [f32; 3],
    pub gyro_pc_active: bool,
    pub rumble_l: u8,
    pub rumble_r: u8,
    pub telemetry: TelemetryState,
}

impl Default for BridgeStatus {
    fn default() -> Self {
        Self {
            running: true,
            state_text: "Iniciando...".into(),
            connected: false,
            rate_hz: 0.0,
            battery: 0,
            dsu_clients: 0,
            dsu_error: None,
            vigem_status: "Conectando ViGEmBus...".into(),
            vigem_ok: false,
            gyro_bias: [0.0; 3],
            gyro_pc_active: false,
            rumble_l: 0,
            rumble_r: 0,
            telemetry: TelemetryState::default(),
        }
    }
}

pub struct BridgeControl {
    pub config: Mutex<BridgeConfig>,
    pub status: Mutex<BridgeStatus>,
    pub running: AtomicBool,
    pub shutdown: AtomicBool,
    pub request_calibrate: AtomicBool,
    pub request_test_rumble: AtomicBool,
}

impl BridgeControl {
    pub fn new(cfg: BridgeConfig) -> Arc<Self> {
        Arc::new(Self {
            config: Mutex::new(cfg),
            status: Mutex::new(BridgeStatus::default()),
            running: AtomicBool::new(true),
            shutdown: AtomicBool::new(false),
            request_calibrate: AtomicBool::new(false),
            request_test_rumble: AtomicBool::new(false),
        })
    }
}

fn is_gyro_trigger_active(
    st: &TelemetryState,
    trigger: GyroTrigger,
    toggle_state: bool,
    paddle_pause: bool,
) -> bool {
    if paddle_pause {
        return false;
    }
    match trigger {
        GyroTrigger::AlwaysOn => (st.btns & BTN_CAPTURE) == 0,
        GyroTrigger::ToggleCapture => toggle_state,
        GyroTrigger::LeftTrigger => st.l2 >= 45 || (st.btns & BTN_L2_DIG) != 0,
        GyroTrigger::RightTrigger => st.r2 >= 45 || (st.btns & BTN_R2_DIG) != 0,
        GyroTrigger::LeftBumper => (st.btns & BTN_L1) != 0,
        GyroTrigger::RightBumper => (st.btns & BTN_R1) != 0,
    }
}

pub fn spawn_bridge_thread(ctrl: Arc<BridgeControl>) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("pro3-bridge-200hz".into())
        .spawn(move || bridge_loop(ctrl))
        .expect("Failed to spawn bridge thread")
}

fn bridge_loop(ctrl: Arc<BridgeControl>) {
    let mut api = match HidApi::new() {
        Ok(a) => a,
        Err(e) => {
            if let Ok(mut s) = ctrl.status.lock() {
                s.state_text = format!("Error HIDAPI: {e}");
            }
            return;
        }
    };

    let mut dsu: Option<DsuServer> = None;
    let mut dsu_port_bound: u16 = 0;
    let mut vigem: Option<VigemBridge> = None;
    let mut vigem_mode_active: Option<PadMode> = None;
    let mut vigem_retry_at = Instant::now();

    let mut pressed_vks: HashSet<u16> = HashSet::new();
    let epoch = Instant::now();

    while !ctrl.shutdown.load(Ordering::Relaxed) {
        if !ctrl.running.load(Ordering::Relaxed) {
            // Release virtual pad & keys while paused
            vigem = None;
            vigem_mode_active = None;
            dsu = None;
            for vk in pressed_vks.drain() {
                send_key_state(vk, false);
            }
            if let Ok(mut s) = ctrl.status.lock() {
                s.running = false;
                s.connected = false;
                s.rate_hz = 0.0;
                s.state_text = "Detenido".into();
                s.vigem_ok = false;
                s.vigem_status = "Inactivo".into();
            }
            thread::sleep(Duration::from_millis(100));
            continue;
        }

        let cfg_snap = ctrl.config.lock().map(|c| c.clone()).unwrap_or_default();

        // Ensure DSU server matches config
        if cfg_snap.dsu_enabled {
            if dsu.is_none() || dsu_port_bound != cfg_snap.dsu_port {
                dsu = None;
                match DsuServer::bind(cfg_snap.dsu_port, [0x8B, 0x17, 0xD0, 0x03, 0x24, 0x00]) {
                    Ok(srv) => {
                        dsu_port_bound = cfg_snap.dsu_port;
                        dsu = Some(srv);
                        if let Ok(mut s) = ctrl.status.lock() {
                            s.dsu_error = None;
                        }
                    }
                    Err(e) => {
                        if let Ok(mut s) = ctrl.status.lock() {
                            s.dsu_error = Some(format!("Puerto {} ocupado ({e})", cfg_snap.dsu_port));
                        }
                    }
                }
            }
        } else {
            dsu = None;
        }

        // Ensure ViGEm virtual pad matches config
        ensure_vigem(
            &ctrl,
            &cfg_snap,
            &mut vigem,
            &mut vigem_mode_active,
            &mut vigem_retry_at,
        );

        // Connect to dongle
        let (dev, mac) = match try_connect_dongle(&mut api) {
            ConnectOutcome::Connected { dev, mac } => (dev, mac),
            ConnectOutcome::SwitchingMode => {
                if let Ok(mut s) = ctrl.status.lock() {
                    s.running = true;
                    s.connected = false;
                    s.state_text = "Activando modo telemetría 200 Hz (2DC8:600A)...".into();
                }
                thread::sleep(Duration::from_millis(150));
                continue;
            }
            ConnectOutcome::WaitingForController => {
                if let Ok(mut s) = ctrl.status.lock() {
                    s.running = true;
                    s.connected = false;
                    s.state_text = "Dongle en espera — encendé el mando o tocá un botón".into();
                }
                if let Some(srv) = dsu.as_mut() {
                    srv.connected = false;
                    srv.poll_incoming();
                }
                thread::sleep(Duration::from_millis(250));
                continue;
            }
            ConnectOutcome::DongleNotFound => {
                if let Ok(mut s) = ctrl.status.lock() {
                    s.running = true;
                    s.connected = false;
                    s.state_text = "Conectá el dongle 2.4 GHz del 8BitDo Pro 3".into();
                }
                if let Some(srv) = dsu.as_mut() {
                    srv.connected = false;
                    srv.poll_incoming();
                }
                thread::sleep(Duration::from_millis(350));
                continue;
            }
        };

        if let Some(srv) = dsu.as_mut() {
            srv.mac = mac;
            srv.connected = true;
        }

        // Initial gyro calibration
        if let Ok(mut s) = ctrl.status.lock() {
            s.state_text = "Calibrando giroscopio (dejá el mando quieto 0.4s)...".into();
        }
        let (mut gyro_bias, _) = calibrate_gyro(&dev, Duration::from_millis(420));

        if let Ok(mut s) = ctrl.status.lock() {
            s.running = true;
            s.connected = true;
            s.gyro_bias = gyro_bias;
            s.state_text = "Conectado (2.4 GHz · 200 Hz)".into();
        }

        // Hot 200 Hz event-driven loop
        let mut buf = [0u8; 64];
        let mut last_pkt_t = Instant::now();
        let mut hz_win_t = Instant::now();
        let mut pkt_count: u32 = 0;
        let mut rate_hz: f32 = 0.0;
        let mut dead_imu_count: u32 = 0;
        let mut last_ui_sync = Instant::now();

        let mut acc_mx: f32 = 0.0;
        let mut acc_my: f32 = 0.0;
        let mut toggle_state = true;
        let mut prev_capture = false;
        let mut recal_hold_start: Option<Instant> = None;
        let mut prev_paddle_recal = false;

        // Rumble tracking
        let mut cur_rumble: (u8, u8) = (0, 0);
        let mut sent_rumble: (u8, u8) = (0, 0);
        let mut last_rumble_sent = Instant::now() - Duration::from_secs(5);
        let mut test_rumble_until: Option<Instant> = None;

        while !ctrl.shutdown.load(Ordering::Relaxed) && ctrl.running.load(Ordering::Relaxed) {
            let n = match dev.read_timeout(&mut buf, 5) {
                Ok(n) => n,
                Err(_) => {
                    // Dongle disconnected
                    break;
                }
            };

            let cfg = ctrl.config.lock().map(|c| c.clone()).unwrap_or_default();

            // Hot-reload ViGEmBus target or DSU port if user changed them in UI
            ensure_vigem(
                &ctrl,
                &cfg,
                &mut vigem,
                &mut vigem_mode_active,
                &mut vigem_retry_at,
            );
            if cfg.dsu_enabled {
                if dsu.is_none() || dsu_port_bound != cfg.dsu_port {
                    dsu = None;
                    if let Ok(srv) = DsuServer::bind(cfg.dsu_port, mac) {
                        dsu_port_bound = cfg.dsu_port;
                        dsu = Some(srv);
                    }
                }
            } else {
                dsu = None;
            }

            // Manual calibration button from UI
            if ctrl.request_calibrate.swap(false, Ordering::Relaxed) {
                let (b, ok) = calibrate_gyro(&dev, Duration::from_millis(420));
                if ok {
                    gyro_bias = b;
                }
                last_pkt_t = Instant::now();
                continue;
            }

            // Test rumble pulse from UI
            if ctrl.request_test_rumble.swap(false, Ordering::Relaxed) {
                test_rumble_until = Some(Instant::now() + Duration::from_millis(320));
            }

            if n < 42 {
                continue;
            }
            let Some(st) = TelemetryState::parse(&buf[..n]) else {
                continue;
            };

            let now = Instant::now();
            let dt = now.duration_since(last_pkt_t).as_secs_f32().clamp(0.001, 0.05);
            last_pkt_t = now;
            pkt_count += 1;

            // Auto-heal IMU if controller slept/reconnected
            if !st.imu_is_live() {
                dead_imu_count += 1;
                if dead_imu_count == 60 {
                    send_enable_imu_cmd(&dev);
                    dead_imu_count = 0;
                }
            } else {
                dead_imu_count = 0;
            }

            // Extra paddles state
            let paddle_pairs = [
                ((st.btns & BTN_P1) != 0, cfg.paddle_p1),
                ((st.btns & BTN_P2) != 0, cfg.paddle_p2),
                ((st.btns & BTN_P3) != 0, cfg.paddle_p3),
                ((st.btns & BTN_P4) != 0, cfg.paddle_p4),
            ];
            let mut active_paddles = [PaddleAction::None; 4];
            let mut active_count = 0usize;
            let mut paddle_pause_gyro = false;
            let mut paddle_recal_pressed = false;
            let mut wanted_vks: HashSet<u16> = HashSet::new();

            for (pressed, act) in paddle_pairs {
                if pressed && act != PaddleAction::None {
                    active_paddles[active_count] = act;
                    active_count += 1;
                    if act == PaddleAction::PauseGyro {
                        paddle_pause_gyro = true;
                    }
                    if act == PaddleAction::RecalibrateGyro {
                        paddle_recal_pressed = true;
                    }
                    if let Some(vk) = act.virtual_key() {
                        wanted_vks.insert(vk);
                    }
                }
            }

            // Sync keyboard SendInput edges for extra paddles
            for &vk in &wanted_vks {
                if !pressed_vks.contains(&vk) {
                    send_key_state(vk, true);
                    pressed_vks.insert(vk);
                }
            }
            pressed_vks.retain(|&vk| {
                if !wanted_vks.contains(&vk) {
                    send_key_state(vk, false);
                    false
                } else {
                    true
                }
            });

            // Trigger recalibration via paddle rising edge or Minus+Plus held 0.5s
            if paddle_recal_pressed && !prev_paddle_recal {
                let (b, ok) = calibrate_gyro(&dev, Duration::from_millis(400));
                if ok {
                    gyro_bias = b;
                }
                prev_paddle_recal = true;
                last_pkt_t = Instant::now();
                continue;
            }
            prev_paddle_recal = paddle_recal_pressed;

            if (st.btns & BTN_MINUS) != 0 && (st.btns & BTN_PLUS) != 0 {
                match recal_hold_start {
                    None => recal_hold_start = Some(now),
                    Some(t) if now.duration_since(t) >= Duration::from_millis(500) => {
                        let (b, ok) = calibrate_gyro(&dev, Duration::from_millis(400));
                        if ok {
                            gyro_bias = b;
                        }
                        recal_hold_start = None;
                        last_pkt_t = Instant::now();
                        continue;
                    }
                    _ => {}
                }
            } else {
                recal_hold_start = None;
            }

            // Capture toggle for Gyro PC mode
            let cap = (st.btns & BTN_CAPTURE) != 0;
            if cap && !prev_capture && cfg.gyro_trigger == GyroTrigger::ToggleCapture {
                toggle_state = !toggle_state;
            }
            prev_capture = cap;

            // 1) DSU server push
            if let Some(srv) = dsu.as_mut() {
                let ts_us = epoch.elapsed().as_micros() as u64;
                srv.push_state(&st, gyro_bias, ts_us);
                if let Ok(mut dr) = srv.dsu_rumble.lock() {
                    if dr.updated {
                        cur_rumble = (dr.large, dr.small);
                        dr.updated = false;
                    }
                }
            }

            // 2) Optional Gyro PC aiming (Mouse or Right Stick)
            let gx = st.gx_dps - gyro_bias[0];
            let gy = st.gy_dps - gyro_bias[1];
            let gz = st.gz_dps - gyro_bias[2];
            let mut horiz_dps = (-gz) + cfg.gyro_roll_mix * gx;
            let mut vert_dps = -gy;
            if horiz_dps.abs() < cfg.gyro_deadzone {
                horiz_dps = 0.0;
            }
            if vert_dps.abs() < cfg.gyro_deadzone {
                vert_dps = 0.0;
            }
            if cfg.invert_x {
                horiz_dps = -horiz_dps;
            }
            if cfg.invert_y {
                vert_dps = -vert_dps;
            }

            let gyro_pc_active = cfg.gyro_pc_mode != GyroPcMode::Off
                && is_gyro_trigger_active(&st, cfg.gyro_trigger, toggle_state, paddle_pause_gyro);

            let mut rx_ov = None;
            let mut ry_ov = None;
            if gyro_pc_active {
                match cfg.gyro_pc_mode {
                    GyroPcMode::Mouse => {
                        let scale = cfg.gyro_sens * 12.0 * dt;
                        acc_mx += horiz_dps * scale;
                        acc_my += (-vert_dps) * scale;
                        let dx = acc_mx.trunc() as i32;
                        let dy = acc_my.trunc() as i32;
                        if dx != 0 || dy != 0 {
                            acc_mx -= dx as f32;
                            acc_my -= dy as f32;
                            send_mouse_delta(dx, dy);
                        }
                    }
                    GyroPcMode::RightStick => {
                        let base_rx = ((st.rx as f32) - 127.5) / 127.5;
                        let base_ry = ((st.ry as f32) - 127.5) / 127.5;
                        let gain = cfg.gyro_sens / 120.0;
                        rx_ov = Some((base_rx + horiz_dps * gain).clamp(-1.0, 1.0));
                        ry_ov = Some((base_ry + (-vert_dps) * gain).clamp(-1.0, 1.0));
                    }
                    GyroPcMode::Off => {}
                }
            } else {
                acc_mx = 0.0;
                acc_my = 0.0;
            }

            // 3) Virtual Controller (ViGEmBus DS4 with Native Gyro OR Xbox 360)
            if let Some(vb) = vigem.as_ref() {
                vb.update(
                    &st,
                    gyro_bias,
                    cfg.layout,
                    &active_paddles[..active_count],
                    rx_ov,
                    ry_ov,
                );
                if let Some(r) = vb.poll_rumble() {
                    cur_rumble = r;
                }
            }

            // 4) Flush rumble to 2.4 GHz dongle
            let target_rumble = if let Some(until) = test_rumble_until {
                if now < until {
                    (180u8, 180u8)
                } else {
                    test_rumble_until = None;
                    (0u8, 0u8)
                }
            } else if cfg.rumble_enabled {
                let s = cfg.rumble_strength.clamp(0.0, 1.0);
                (
                    ((cur_rumble.0 as f32) * s).round() as u8,
                    ((cur_rumble.1 as f32) * s).round() as u8,
                )
            } else {
                (0u8, 0u8)
            };

            let changed = target_rumble != sent_rumble;
            let stopping = target_rumble == (0, 0) && sent_rumble != (0, 0);
            let need_refresh = (target_rumble.0 > 0 || target_rumble.1 > 0)
                && now.duration_since(last_rumble_sent) >= Duration::from_millis(1200);
            if stopping || (changed && now.duration_since(last_rumble_sent) >= Duration::from_millis(15)) || need_refresh {
                send_dongle_rumble(&dev, target_rumble.0, target_rumble.1, 2000);
                sent_rumble = target_rumble;
                last_rumble_sent = now;
            }

            // 5) Update GUI snapshot (~60 Hz max)
            let dt_win = now.duration_since(hz_win_t).as_secs_f32();
            if dt_win >= 0.75 {
                rate_hz = (pkt_count as f32) / dt_win;
                pkt_count = 0;
                hz_win_t = now;
            }
            if now.duration_since(last_ui_sync) >= Duration::from_millis(16) {
                last_ui_sync = now;
                if let Ok(mut s) = ctrl.status.lock() {
                    s.running = true;
                    s.connected = true;
                    s.rate_hz = rate_hz;
                    s.battery = st.battery;
                    s.dsu_clients = dsu.as_ref().map(|d| d.client_count()).unwrap_or(0);
                    s.gyro_bias = gyro_bias;
                    s.gyro_pc_active = gyro_pc_active;
                    s.rumble_l = sent_rumble.0;
                    s.rumble_r = sent_rumble.1;
                    s.telemetry = st;
                    s.state_text = "Conectado (2.4 GHz · 200 Hz)".into();
                }
            }
        }

        // Clean stop rumble on disconnect/pause
        send_dongle_rumble(&dev, 0, 0, 0);
        for vk in pressed_vks.drain() {
            send_key_state(vk, false);
        }
    }
}

fn ensure_vigem(
    ctrl: &Arc<BridgeControl>,
    cfg: &BridgeConfig,
    vigem: &mut Option<VigemBridge>,
    active_mode: &mut Option<PadMode>,
    retry_at: &mut Instant,
) {
    if cfg.pad_mode == PadMode::None {
        if vigem.is_some() || *active_mode != Some(PadMode::None) {
            *vigem = None;
            *active_mode = Some(PadMode::None);
            if let Ok(mut s) = ctrl.status.lock() {
                s.vigem_ok = false;
                s.vigem_status = "Desactivado (Solo DSU)".into();
            }
        }
        return;
    }

    if vigem.as_ref().is_some_and(|v| v.mode() == cfg.pad_mode) {
        return;
    }
    if Instant::now() < *retry_at {
        return;
    }

    // Unplug previous target before plugging new one
    *vigem = None;
    match VigemBridge::connect(cfg.pad_mode) {
        Ok(vb) => {
            *active_mode = Some(cfg.pad_mode);
            *vigem = Some(vb);
            if let Ok(mut s) = ctrl.status.lock() {
                s.vigem_ok = true;
                s.vigem_status = match cfg.pad_mode {
                    PadMode::Ds4NativeGyro => "DualShock 4 (Gyro Nativo Steam)".into(),
                    PadMode::Xbox360 => "Xbox 360 (XInput)".into(),
                    PadMode::None => "Desactivado".into(),
                };
            }
        }
        Err(e) => {
            *retry_at = Instant::now() + Duration::from_secs(3);
            if let Ok(mut s) = ctrl.status.lock() {
                s.vigem_ok = false;
                s.vigem_status = e;
            }
        }
    }
}
