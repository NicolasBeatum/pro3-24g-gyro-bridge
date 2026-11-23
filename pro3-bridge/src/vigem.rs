use crate::config::{ButtonLayout, PadMode, PaddleAction};
use crate::protocol::*;
use std::time::{Duration, Instant};
use vigem_rust::{
    Client, Ds4ButtonFlags, Ds4Buttons, Ds4Dpad, Ds4Notification, Ds4ReportEx, Ds4SpecialButton,
    DualShock4, NotificationReceiver, TargetHandle, X360Button, X360Notification, X360Report,
    Xbox360,
};

fn clamp_i16(v: f32) -> i16 {
    v.round().clamp(i16::MIN as f32, i16::MAX as f32) as i16
}

fn u8_axis_to_i16(v: u8, invert: bool) -> i16 {
    let f = ((v as f32) - 127.5) / 127.5;
    let f = if invert { -f } else { f };
    clamp_i16(f * 32767.0)
}

enum ActiveTarget {
    Ds4 {
        handle: TargetHandle<DualShock4>,
        notif: Option<NotificationReceiver<Ds4Notification>>,
    },
    X360 {
        handle: TargetHandle<Xbox360>,
        notif: Option<NotificationReceiver<X360Notification>>,
    },
}

pub struct VigemBridge {
    _client: Client,
    target: ActiveTarget,
    mode: PadMode,
    start_instant: Instant,
}

impl VigemBridge {
    pub fn connect(mode: PadMode) -> Result<Self, String> {
        let client = Client::connect().map_err(|e| format!("ViGEmBus not available: {e}"))?;
        let target = match mode {
            PadMode::Ds4NativeGyro => {
                let pending = client
                    .new_ds4_target()
                    .plug()
                    .map_err(|e| format!("Could not create virtual DualShock 4: {e}"))?;
                let handle = pending
                    .wait_for_ready_timeout(Duration::from_secs(2))
                    .unwrap_or_else(|_| pending.assume_ready());
                let notif = handle.register_notification().ok();
                ActiveTarget::Ds4 { handle, notif }
            }
            PadMode::Xbox360 => {
                let pending = client
                    .new_x360_target()
                    .plug()
                    .map_err(|e| format!("Could not create virtual Xbox 360 pad: {e}"))?;
                let handle = pending
                    .wait_for_ready_timeout(Duration::from_secs(2))
                    .unwrap_or_else(|_| pending.assume_ready());
                let notif = handle.register_notification().ok();
                ActiveTarget::X360 { handle, notif }
            }
            PadMode::None => return Err("No virtual pad selected".into()),
        };
        Ok(Self {
            _client: client,
            target,
            mode,
            start_instant: Instant::now(),
        })
    }

    pub fn mode(&self) -> PadMode {
        self.mode
    }

    /// Drains any pending rumble notifications from Windows/Steam and returns the latest (large, small).
    pub fn poll_rumble(&self) -> Option<(u8, u8)> {
        let mut latest = None;
        match &self.target {
            ActiveTarget::Ds4 { notif: Some(rx), .. } => {
                while let Ok(n) = rx.try_recv() {
                    latest = Some((n.large_motor, n.small_motor));
                }
            }
            ActiveTarget::X360 { notif: Some(rx), .. } => {
                while let Ok(n) = rx.try_recv() {
                    latest = Some((n.large_motor, n.small_motor));
                }
            }
            _ => {}
        }
        latest
    }

    pub fn update(
        &self,
        st: &TelemetryState,
        gyro_bias: [f32; 3],
        layout: ButtonLayout,
        active_paddles: &[PaddleAction],
        rx_override: Option<f32>,
        ry_override: Option<f32>,
    ) {
        let mut btns = st.btns;
        let mut extra_touchpad = false;
        for &act in active_paddles {
            match act {
                PaddleAction::TouchpadClick => extra_touchpad = true,
                PaddleAction::PadL3 => btns |= BTN_L3,
                PaddleAction::PadR3 => btns |= BTN_R3,
                PaddleAction::PadSouth => btns |= BTN_SOUTH,
                PaddleAction::PadEast => btns |= BTN_EAST,
                PaddleAction::PadWest => btns |= BTN_WEST,
                PaddleAction::PadNorth => btns |= BTN_NORTH,
                PaddleAction::PadL1 => btns |= BTN_L1,
                PaddleAction::PadR1 => btns |= BTN_R1,
                _ => {}
            }
        }

        match &self.target {
            ActiveTarget::Ds4 { handle, .. } => {
                let mut rep = Ds4ReportEx::default();
                let mut flags = Ds4ButtonFlags::empty();
                if layout == ButtonLayout::Nintendo {
                    if (btns & BTN_EAST) != 0 { flags |= Ds4ButtonFlags::CROSS; }
                    if (btns & BTN_SOUTH) != 0 { flags |= Ds4ButtonFlags::CIRCLE; }
                    if (btns & BTN_NORTH) != 0 { flags |= Ds4ButtonFlags::SQUARE; }
                    if (btns & BTN_WEST) != 0 { flags |= Ds4ButtonFlags::TRIANGLE; }
                } else {
                    if (btns & BTN_SOUTH) != 0 { flags |= Ds4ButtonFlags::CROSS; }
                    if (btns & BTN_EAST) != 0 { flags |= Ds4ButtonFlags::CIRCLE; }
                    if (btns & BTN_WEST) != 0 { flags |= Ds4ButtonFlags::SQUARE; }
                    if (btns & BTN_NORTH) != 0 { flags |= Ds4ButtonFlags::TRIANGLE; }
                }
                if (btns & BTN_L1) != 0 { flags |= Ds4ButtonFlags::SHOULDER_LEFT; }
                if (btns & BTN_R1) != 0 { flags |= Ds4ButtonFlags::SHOULDER_RIGHT; }
                if (btns & BTN_L2_DIG) != 0 || st.l2 > 10 { flags |= Ds4ButtonFlags::TRIGGER_LEFT; }
                if (btns & BTN_R2_DIG) != 0 || st.r2 > 10 { flags |= Ds4ButtonFlags::TRIGGER_RIGHT; }
                if (btns & BTN_L3) != 0 { flags |= Ds4ButtonFlags::THUMB_LEFT; }
                if (btns & BTN_R3) != 0 { flags |= Ds4ButtonFlags::THUMB_RIGHT; }
                if (btns & BTN_MINUS) != 0 { flags |= Ds4ButtonFlags::SHARE; }
                if (btns & BTN_PLUS) != 0 { flags |= Ds4ButtonFlags::OPTIONS; }

                let up = (btns & BTN_DUP) != 0;
                let dn = (btns & BTN_DDOWN) != 0;
                let lf = (btns & BTN_DLEFT) != 0;
                let rt = (btns & BTN_DRIGHT) != 0;
                let dpad = match (up, dn, lf, rt) {
                    (true, false, false, true) => Ds4Dpad::NorthEast,
                    (true, false, true, false) => Ds4Dpad::NorthWest,
                    (false, true, false, true) => Ds4Dpad::SouthEast,
                    (false, true, true, false) => Ds4Dpad::SouthWest,
                    (true, false, _, _) => Ds4Dpad::North,
                    (false, true, _, _) => Ds4Dpad::South,
                    (_, _, true, false) => Ds4Dpad::West,
                    (_, _, false, true) => Ds4Dpad::East,
                    _ => Ds4Dpad::Neutral,
                };
                rep.buttons = Ds4Buttons::from(flags).with_dpad(dpad);

                let mut special = Ds4SpecialButton::empty();
                if (btns & BTN_HOME) != 0 { special |= Ds4SpecialButton::PS; }
                if (btns & BTN_CAPTURE) != 0 || extra_touchpad {
                    special |= Ds4SpecialButton::TOUCHPAD;
                }
                rep.special = special;

                rep.thumb_lx = st.lx;
                rep.thumb_ly = st.ly;
                rep.thumb_rx = match rx_override {
                    Some(v) => (128.0 + v.clamp(-1.0, 1.0) * 127.0).round() as u8,
                    None => st.rx,
                };
                rep.thumb_ry = match ry_override {
                    Some(v) => (128.0 + v.clamp(-1.0, 1.0) * 127.0).round() as u8,
                    None => st.ry,
                };
                rep.trigger_left = st.l2;
                rep.trigger_right = st.r2;

                // Sensor timestamp in 5.33us units (187500 ticks/sec)
                let ts = ((self.start_instant.elapsed().as_micros() * 3) / 16) as u16;
                rep.timestamp = ts;

                // Exact SDL/DS4 right-handed sensor frame
                let gx = st.gx_dps - gyro_bias[0];
                let gy = st.gy_dps - gyro_bias[1];
                let gz = st.gz_dps - gyro_bias[2];
                let gyro_scale = 32767.0 / 2000.0;
                rep.gyro_x = clamp_i16(-gy * gyro_scale);
                rep.gyro_y = clamp_i16(gz * gyro_scale);
                rep.gyro_z = clamp_i16(-gx * gyro_scale);

                rep.accel_x = clamp_i16(-st.ay_g * 8192.0);
                rep.accel_y = clamp_i16(st.az_g * 8192.0);
                rep.accel_z = clamp_i16(-st.ax_g * 8192.0);

                let bat_code = (st.battery / 10).min(11);
                rep.battery_lvl = st.battery;
                rep.battery_lvl_special = 0x10 | bat_code; // USB connected + battery level

                let _ = handle.update_ex(&rep);
            }
            ActiveTarget::X360 { handle, .. } => {
                let mut rep = X360Report::default();
                let mut b = X360Button::empty();
                if layout == ButtonLayout::Nintendo {
                    if (btns & BTN_EAST) != 0 { b |= X360Button::A; }
                    if (btns & BTN_SOUTH) != 0 { b |= X360Button::B; }
                    if (btns & BTN_NORTH) != 0 { b |= X360Button::X; }
                    if (btns & BTN_WEST) != 0 { b |= X360Button::Y; }
                } else {
                    if (btns & BTN_SOUTH) != 0 { b |= X360Button::A; }
                    if (btns & BTN_EAST) != 0 { b |= X360Button::B; }
                    if (btns & BTN_WEST) != 0 { b |= X360Button::X; }
                    if (btns & BTN_NORTH) != 0 { b |= X360Button::Y; }
                }
                if (btns & BTN_DUP) != 0 { b |= X360Button::DPAD_UP; }
                if (btns & BTN_DDOWN) != 0 { b |= X360Button::DPAD_DOWN; }
                if (btns & BTN_DLEFT) != 0 { b |= X360Button::DPAD_LEFT; }
                if (btns & BTN_DRIGHT) != 0 { b |= X360Button::DPAD_RIGHT; }
                if (btns & BTN_L1) != 0 { b |= X360Button::LEFT_SHOULDER; }
                if (btns & BTN_R1) != 0 { b |= X360Button::RIGHT_SHOULDER; }
                if (btns & BTN_L3) != 0 { b |= X360Button::LEFT_THUMB; }
                if (btns & BTN_R3) != 0 { b |= X360Button::RIGHT_THUMB; }
                if (btns & BTN_MINUS) != 0 { b |= X360Button::BACK; }
                if (btns & BTN_PLUS) != 0 { b |= X360Button::START; }
                if (btns & BTN_HOME) != 0 { b |= X360Button::GUIDE; }

                rep.buttons = b;
                rep.left_trigger = st.l2;
                rep.right_trigger = st.r2;
                rep.thumb_lx = u8_axis_to_i16(st.lx, false);
                rep.thumb_ly = u8_axis_to_i16(st.ly, true);
                rep.thumb_rx = match rx_override {
                    Some(v) => clamp_i16(v.clamp(-1.0, 1.0) * 32767.0),
                    None => u8_axis_to_i16(st.rx, false),
                };
                rep.thumb_ry = match ry_override {
                    Some(v) => clamp_i16((-v).clamp(-1.0, 1.0) * 32767.0),
                    None => u8_axis_to_i16(st.ry, true),
                };
                let _ = handle.update(&rep);
            }
        }
    }
}
