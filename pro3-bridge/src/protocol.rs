use hidapi::{HidApi, HidDevice};
use std::time::{Duration, Instant};

pub const VID_8BITDO: u16 = 0x2DC8;
pub const PID_XINPUT: u16 = 0x310B;
pub const PID_DINPUT: u16 = 0x600A;
pub const PID_IDLE: u16 = 0x3109;

pub const ACCEL_SCALE: f32 = 4096.0;
pub const GYRO_DPS_SCALE: f32 = 2000.0 / 32767.0;

// Exact 32-bit button bitmask verified from 8BitDo Pro 3 adapter firmware v1.02
pub const BTN_PLUS: u32 = 1 << 0;
pub const BTN_L3: u32 = 1 << 1;
pub const BTN_R3: u32 = 1 << 2;
pub const BTN_MINUS: u32 = 1 << 3;
pub const BTN_NORTH: u32 = 1 << 4; // Xbox Y / Switch X / PS Triangle
pub const BTN_WEST: u32 = 1 << 5; // Xbox X / Switch Y / PS Square
pub const BTN_DRIGHT: u32 = 1 << 6;
pub const BTN_DLEFT: u32 = 1 << 7;
pub const BTN_DDOWN: u32 = 1 << 8;
pub const BTN_DUP: u32 = 1 << 9;
pub const BTN_L1: u32 = 1 << 10;
pub const BTN_R1: u32 = 1 << 11;
pub const BTN_SOUTH: u32 = 1 << 12; // Xbox A / Switch B / PS Cross
pub const BTN_EAST: u32 = 1 << 13; // Xbox B / Switch A / PS Circle
pub const BTN_L2_DIG: u32 = 1 << 14;
pub const BTN_R2_DIG: u32 = 1 << 15;
pub const BTN_CAPTURE: u32 = 1 << 16; // Star / Capture
pub const BTN_HOME: u32 = 1 << 17; // Home / Guide / PS

// Extra paddles & hardware buttons (from radio RX handler 0x0E756..0x0E7EA)
pub const BTN_P3: u32 = 1 << 21; // KEY_P3_MAP
pub const BTN_P1: u32 = 1 << 25; // Extra paddle 1
pub const BTN_P2: u32 = 1 << 26; // Extra paddle 2
pub const BTN_P4: u32 = 1 << 30; // KEY_P4_MAP

#[derive(Debug, Clone, Copy, Default)]
pub struct TelemetryState {
    pub btns: u32,
    pub lx: u8,
    pub ly: u8,
    pub rx: u8,
    pub ry: u8,
    pub l2: u8,
    pub r2: u8,
    pub ax_g: f32,
    pub ay_g: f32,
    pub az_g: f32,
    pub gx_dps: f32,
    pub gy_dps: f32,
    pub gz_dps: f32,
    pub battery: u8,
}

impl TelemetryState {
    pub fn parse(pkt: &[u8]) -> Option<Self> {
        if pkt.len() < 42 || pkt[0] != 0x02 || pkt[4] != 0x03 || pkt[6] != 0x18 {
            return None;
        }
        let btns = u32::from_le_bytes([pkt[18], pkt[19], pkt[20], pkt[21]]);
        let lx = pkt[22];
        let ly = pkt[23];
        let rx = pkt[24];
        let ry = pkt[25];
        // Verified against SDL_hidapi_8bitdo.c & firmware 0x0B7B6: byte 26 = R2, byte 27 = L2
        let r2 = pkt[26];
        let l2 = pkt[27];

        let ax = i16::from_le_bytes([pkt[28], pkt[29]]) as f32;
        let ay = i16::from_le_bytes([pkt[30], pkt[31]]) as f32;
        let az = i16::from_le_bytes([pkt[32], pkt[33]]) as f32;
        let gx = i16::from_le_bytes([pkt[34], pkt[35]]) as f32;
        let gy = i16::from_le_bytes([pkt[36], pkt[37]]) as f32;
        let gz = i16::from_le_bytes([pkt[38], pkt[39]]) as f32;
        let battery = pkt[41];

        Some(Self {
            btns,
            lx,
            ly,
            rx,
            ry,
            l2,
            r2,
            ax_g: ax / ACCEL_SCALE,
            ay_g: ay / ACCEL_SCALE,
            az_g: az / ACCEL_SCALE,
            gx_dps: gx * GYRO_DPS_SCALE,
            gy_dps: gy * GYRO_DPS_SCALE,
            gz_dps: gz * GYRO_DPS_SCALE,
            battery,
        })
    }

    pub fn imu_is_live(&self) -> bool {
        let mag = (self.ax_g * self.ax_g + self.ay_g * self.ay_g + self.az_g * self.az_g).sqrt();
        (0.35..=3.0).contains(&mag)
    }
}

pub fn parse_mac_from_serial(serial: Option<&str>) -> [u8; 6] {
    if let Some(s) = serial {
        let hex: String = s.chars().filter(|c| c.is_ascii_hexdigit()).collect();
        if hex.len() >= 12 {
            let mut out = [0u8; 6];
            let mut ok = true;
            for i in 0..6 {
                if let Ok(v) = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16) {
                    out[i] = v;
                } else {
                    ok = false;
                    break;
                }
            }
            if ok {
                return out;
            }
        }
    }
    [0x8B, 0x17, 0xD0, 0x03, 0x24, 0x00]
}

/// Sends WIRELESS_ENTER_TEST_MODE_CMD ([0x81, 0x06, 0x08, 0x00]) to enable 200 Hz IMU telemetry.
pub fn send_enable_imu_cmd(dev: &HidDevice) {
    let mut pkt = [0u8; 64];
    pkt[0] = 0x81;
    pkt[1] = 0x06;
    pkt[2] = 0x08;
    pkt[3] = 0x00;
    let _ = dev.write(&pkt);
}

/// Sends rumble via 2DC8:600A CUSTOM_CMD 0x08 (Report ID 0x81).
/// duration_ms = 0 or (large == 0 && small == 0) stops both motors immediately.
pub fn send_dongle_rumble(dev: &HidDevice, large_motor: u8, small_motor: u8, duration_ms: u16) {
    let dur = if large_motor == 0 && small_motor == 0 {
        0u16
    } else {
        duration_ms.clamp(50, 60000)
    };
    let mut pkt = [0u8; 64];
    pkt[0] = 0x81;
    pkt[1] = 0x04; // CUSTOM_CMD
    pkt[2] = 0x08; // cmd = 8 (rumble)
    pkt[3] = 0x00;
    let d = dur.to_le_bytes();
    pkt[4] = d[0];
    pkt[5] = d[1];
    pkt[6] = large_motor;
    pkt[7] = small_motor;
    let _ = dev.write(&pkt);
}

pub enum ConnectOutcome {
    Connected { dev: HidDevice, mac: [u8; 6] },
    SwitchingMode,
    WaitingForController,
    DongleNotFound,
}

pub fn try_connect_dongle(api: &mut HidApi) -> ConnectOutcome {
    let _ = api.refresh_devices();

    // 1) Check if already in 2DC8:600A
    let dev_600a = api
        .device_list()
        .find(|d| d.vendor_id() == VID_8BITDO && d.product_id() == PID_DINPUT)
        .map(|d| (d.path().to_owned(), d.serial_number().map(|s| s.to_owned())));

    if let Some((path, serial)) = dev_600a {
        if let Ok(dev) = api.open_path(&path) {
            let mac = parse_mac_from_serial(serial.as_deref());
            let mut buf = [0u8; 64];
            let t0 = Instant::now();
            let mut live = false;
            while t0.elapsed() < Duration::from_millis(220) {
                if let Ok(n) = dev.read_timeout(&mut buf, 15) {
                    if n >= 42 {
                        if let Some(st) = TelemetryState::parse(&buf[..n]) {
                            if st.imu_is_live() {
                                live = true;
                                break;
                            }
                        }
                    }
                }
            }
            if live {
                return ConnectOutcome::Connected { dev, mac };
            }
            // Wake controller IMU stream
            send_enable_imu_cmd(&dev);
            std::thread::sleep(Duration::from_millis(320));
            // Drain stale packets
            for _ in 0..200 {
                match dev.read_timeout(&mut buf, 0) {
                    Ok(n) if n > 0 => continue,
                    _ => break,
                }
            }
            let t1 = Instant::now();
            while t1.elapsed() < Duration::from_millis(350) {
                if let Ok(n) = dev.read_timeout(&mut buf, 15) {
                    if n >= 42 {
                        if let Some(st) = TelemetryState::parse(&buf[..n]) {
                            if st.imu_is_live() {
                                return ConnectOutcome::Connected { dev, mac };
                            }
                        }
                    }
                }
            }
            return ConnectOutcome::SwitchingMode;
        }
    }

    // 2) Check XInput mode (2DC8:310B, Usage Page 0xFF7A)
    let dev_310b = api
        .device_list()
        .find(|d| {
            d.vendor_id() == VID_8BITDO
                && d.product_id() == PID_XINPUT
                && d.usage_page() == 0xFF7A
        })
        .map(|d| d.path().to_owned());

    if let Some(path) = dev_310b {
        if let Ok(dev) = api.open_path(&path) {
            send_enable_imu_cmd(&dev);
            drop(dev);
            std::thread::sleep(Duration::from_millis(400));
            return ConnectOutcome::SwitchingMode;
        }
    }

    // 3) Check idle receiver (2DC8:3109)
    let is_idle = api
        .device_list()
        .any(|d| d.vendor_id() == VID_8BITDO && d.product_id() == PID_IDLE);

    if is_idle {
        ConnectOutcome::WaitingForController
    } else {
        ConnectOutcome::DongleNotFound
    }
}

pub fn calibrate_gyro(dev: &HidDevice, duration: Duration) -> ([f32; 3], bool) {
    let mut xs = Vec::with_capacity(128);
    let mut ys = Vec::with_capacity(128);
    let mut zs = Vec::with_capacity(128);
    let mut buf = [0u8; 64];
    let t0 = Instant::now();
    while t0.elapsed() < duration {
        if let Ok(n) = dev.read_timeout(&mut buf, 10) {
            if n >= 42 {
                if let Some(st) = TelemetryState::parse(&buf[..n]) {
                    if st.imu_is_live() {
                        xs.push(st.gx_dps);
                        ys.push(st.gy_dps);
                        zs.push(st.gz_dps);
                    }
                }
            }
        }
    }
    if xs.is_empty() {
        return ([0.0, 0.0, 0.0], false);
    }
    let span = |v: &[f32]| -> f32 {
        let mut lo = f32::INFINITY;
        let mut hi = f32::NEG_INFINITY;
        for &x in v {
            if x < lo {
                lo = x;
            }
            if x > hi {
                hi = x;
            }
        }
        hi - lo
    };
    let max_span = span(&xs).max(span(&ys)).max(span(&zs));
    if max_span > 12.0 {
        return ([0.0, 0.0, 0.0], false);
    }
    let n = xs.len() as f32;
    (
        [
            xs.iter().sum::<f32>() / n,
            ys.iter().sum::<f32>() / n,
            zs.iter().sum::<f32>() / n,
        ],
        true,
    )
}
