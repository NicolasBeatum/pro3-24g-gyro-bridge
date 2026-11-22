use crate::protocol::*;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DSU_PROTOCOL_VERSION: u16 = 1001;
const MSG_VERSION: u32 = 0x100000;
const MSG_PORTS: u32 = 0x100001;
const MSG_DATA: u32 = 0x100002;
const MSG_MOTORS: u32 = 0x110001;
const MSG_RUMBLE: u32 = 0x110002;

fn dsu_battery_code(pct: u8) -> u8 {
    if pct >= 85 {
        0x05
    } else if pct >= 60 {
        0x04
    } else if pct >= 35 {
        0x03
    } else if pct >= 15 {
        0x02
    } else if pct > 0 {
        0x01
    } else {
        0x00
    }
}

struct ClientSub {
    expire: Instant,
    pkt_num: u32,
}

pub struct DsuRumbleSlot {
    pub large: u8,
    pub small: u8,
    pub updated: bool,
}

pub struct DsuServer {
    sock: UdpSocket,
    server_id: u32,
    pub mac: [u8; 6],
    pub connected: bool,
    pub battery: u8,
    clients: HashMap<SocketAddr, ClientSub>,
    pub dsu_rumble: Arc<Mutex<DsuRumbleSlot>>,
}

impl DsuServer {
    pub fn bind(port: u16, mac: [u8; 6]) -> std::io::Result<Self> {
        let sock = UdpSocket::bind(("127.0.0.1", port))?;
        sock.set_nonblocking(true)?;
        // Simple pseudo-random server ID from timestamp
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0x8B17D003);
        let server_id = nanos ^ 0x8B17_D003;
        Ok(Self {
            sock,
            server_id,
            mac,
            connected: false,
            battery: 80,
            clients: HashMap::new(),
            dsu_rumble: Arc::new(Mutex::new(DsuRumbleSlot {
                large: 0,
                small: 0,
                updated: false,
            })),
        })
    }

    fn pack_message(&self, msg_type: u32, payload: &[u8]) -> Vec<u8> {
        let length = (4 + payload.len()) as u16;
        let mut buf = Vec::with_capacity(20 + payload.len());
        buf.extend_from_slice(b"DSUS");
        buf.extend_from_slice(&DSU_PROTOCOL_VERSION.to_le_bytes());
        buf.extend_from_slice(&length.to_le_bytes());
        buf.extend_from_slice(&0u32.to_le_bytes()); // CRC placeholder
        buf.extend_from_slice(&self.server_id.to_le_bytes());
        buf.extend_from_slice(&msg_type.to_le_bytes());
        buf.extend_from_slice(payload);
        let crc = crc32fast::hash(&buf);
        buf[8..12].copy_from_slice(&crc.to_le_bytes());
        buf
    }

    fn slot_header(&self, slot: u8) -> [u8; 11] {
        let mut out = [0u8; 11];
        out[0] = slot;
        if slot == 0 && self.connected {
            out[1] = 2; // connected
            out[2] = 2; // full gyro
            out[3] = 1; // USB / 2.4G
            out[4..10].copy_from_slice(&self.mac);
            out[10] = dsu_battery_code(self.battery);
        }
        out
    }

    /// Non-blocking poll for incoming DSUC client queries & rumble commands.
    pub fn poll_incoming(&mut self) {
        let mut buf = [0u8; 512];
        loop {
            let (n, addr) = match self.sock.recv_from(&mut buf) {
                Ok(v) => v,
                Err(_) => break,
            };
            if n < 20 || &buf[0..4] != b"DSUC" {
                continue;
            }
            let msg_type = u32::from_le_bytes([buf[16], buf[17], buf[18], buf[19]]);
            let body = &buf[20..n];

            match msg_type {
                MSG_VERSION => {
                    let pkt = self.pack_message(MSG_VERSION, &DSU_PROTOCOL_VERSION.to_le_bytes());
                    let _ = self.sock.send_to(&pkt, addr);
                }
                MSG_PORTS => {
                    if body.len() >= 4 {
                        let count = i32::from_le_bytes([body[0], body[1], body[2], body[3]])
                            .clamp(0, 4) as usize;
                        for i in 0..count {
                            if 4 + i < body.len() {
                                let slot = body[4 + i];
                                let mut payload = [0u8; 12];
                                payload[..11].copy_from_slice(&self.slot_header(slot));
                                let pkt = self.pack_message(MSG_PORTS, &payload);
                                let _ = self.sock.send_to(&pkt, addr);
                            }
                        }
                    }
                }
                MSG_DATA => {
                    if body.len() >= 8 {
                        let flags = body[0];
                        let slot = body[1];
                        let req_mac = &body[2..8];
                        let wants_slot0 = flags == 0
                            || ((flags & 1) != 0 && slot == 0)
                            || ((flags & 2) != 0 && req_mac == self.mac);
                        if wants_slot0 {
                            let expire = Instant::now() + Duration::from_secs(5);
                            self.clients
                                .entry(addr)
                                .and_modify(|c| c.expire = expire)
                                .or_insert(ClientSub { expire, pkt_num: 0 });
                        }
                    }
                }
                MSG_MOTORS => {
                    let mut payload = [0u8; 12];
                    payload[..11].copy_from_slice(&self.slot_header(0));
                    payload[11] = 2; // 2 rumble motors
                    let pkt = self.pack_message(MSG_MOTORS, &payload);
                    let _ = self.sock.send_to(&pkt, addr);
                }
                MSG_RUMBLE => {
                    if body.len() >= 10 {
                        let motor_id = body[8];
                        let intensity = body[9];
                        if let Ok(mut r) = self.dsu_rumble.lock() {
                            if motor_id == 0 {
                                r.large = intensity;
                                r.updated = true;
                            } else if motor_id == 1 {
                                r.small = intensity;
                                r.updated = true;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        let now = Instant::now();
        self.clients.retain(|_, c| c.expire >= now);
    }

    pub fn client_count(&self) -> usize {
        self.clients.len()
    }

    pub fn push_state(&mut self, st: &TelemetryState, gyro_bias: [f32; 3], timestamp_us: u64) {
        self.connected = true;
        self.battery = st.battery;
        self.poll_incoming();
        if self.clients.is_empty() {
            return;
        }

        let btns = st.btns;
        let b1: u8 = (if (btns & BTN_DLEFT) != 0 { 0x80 } else { 0 })
            | (if (btns & BTN_DDOWN) != 0 { 0x40 } else { 0 })
            | (if (btns & BTN_DRIGHT) != 0 { 0x20 } else { 0 })
            | (if (btns & BTN_DUP) != 0 { 0x10 } else { 0 })
            | (if (btns & BTN_PLUS) != 0 { 0x08 } else { 0 })
            | (if (btns & BTN_R3) != 0 { 0x04 } else { 0 })
            | (if (btns & BTN_L3) != 0 { 0x02 } else { 0 })
            | (if (btns & BTN_MINUS) != 0 { 0x01 } else { 0 });

        let b2: u8 = (if (btns & BTN_WEST) != 0 { 0x80 } else { 0 })
            | (if (btns & BTN_SOUTH) != 0 { 0x40 } else { 0 })
            | (if (btns & BTN_EAST) != 0 { 0x20 } else { 0 })
            | (if (btns & BTN_NORTH) != 0 { 0x10 } else { 0 })
            | (if (btns & BTN_R1) != 0 { 0x08 } else { 0 })
            | (if (btns & BTN_L1) != 0 { 0x04 } else { 0 })
            | (if (btns & BTN_R2_DIG) != 0 || st.r2 > 20 { 0x02 } else { 0 })
            | (if (btns & BTN_L2_DIG) != 0 || st.l2 > 20 { 0x01 } else { 0 });

        let home_b: u8 = if (btns & BTN_HOME) != 0 { 1 } else { 0 };
        let touch_b: u8 = if (btns & BTN_CAPTURE) != 0 { 1 } else { 0 };

        // DSU Y is +up (invert HID Y)
        let lx = st.lx;
        let ly = 255u8.saturating_sub(st.ly);
        let rx = st.rx;
        let ry = 255u8.saturating_sub(st.ry);

        // Verified against SDL_hidapi_8bitdo.c & Dolphin DualShockUDPClient.cpp
        let dsu_ax = st.ay_g;
        let dsu_ay = -st.az_g;
        let dsu_az = st.ax_g;
        let dsu_pitch = -(st.gy_dps - gyro_bias[1]);
        let dsu_yaw = -(st.gz_dps - gyro_bias[2]);
        let dsu_roll = st.gx_dps - gyro_bias[0];

        let slot_hdr = self.slot_header(0);
        let mut body = [0u8; 80];
        body[0..11].copy_from_slice(&slot_hdr);
        body[11] = 1; // connected
        // body[12..16] = pkt_num (filled per client)
        body[16] = b1;
        body[17] = b2;
        body[18] = home_b;
        body[19] = touch_b;
        body[20] = lx;
        body[21] = ly;
        body[22] = rx;
        body[23] = ry;
        body[24] = if (btns & BTN_DLEFT) != 0 { 255 } else { 0 };
        body[25] = if (btns & BTN_DDOWN) != 0 { 255 } else { 0 };
        body[26] = if (btns & BTN_DRIGHT) != 0 { 255 } else { 0 };
        body[27] = if (btns & BTN_DUP) != 0 { 255 } else { 0 };
        body[28] = if (btns & BTN_WEST) != 0 { 255 } else { 0 };
        body[29] = if (btns & BTN_SOUTH) != 0 { 255 } else { 0 };
        body[30] = if (btns & BTN_EAST) != 0 { 255 } else { 0 };
        body[31] = if (btns & BTN_NORTH) != 0 { 255 } else { 0 };
        body[32] = if (btns & BTN_R1) != 0 { 255 } else { 0 };
        body[33] = if (btns & BTN_L1) != 0 { 255 } else { 0 };
        body[34] = st.r2;
        body[35] = st.l2;
        // body[36..48] = 12 zero touch bytes
        body[48..56].copy_from_slice(&timestamp_us.to_le_bytes());
        body[56..60].copy_from_slice(&dsu_ax.to_le_bytes());
        body[60..64].copy_from_slice(&dsu_ay.to_le_bytes());
        body[64..68].copy_from_slice(&dsu_az.to_le_bytes());
        body[68..72].copy_from_slice(&dsu_pitch.to_le_bytes());
        body[72..76].copy_from_slice(&dsu_yaw.to_le_bytes());
        body[76..80].copy_from_slice(&dsu_roll.to_le_bytes());

        let addrs: Vec<SocketAddr> = self.clients.keys().copied().collect();
        for addr in addrs {
            if let Some(sub) = self.clients.get_mut(&addr) {
                let pnum = sub.pkt_num;
                sub.pkt_num = sub.pkt_num.wrapping_add(1);
                body[12..16].copy_from_slice(&pnum.to_le_bytes());
                let pkt = self.pack_message(MSG_DATA, &body);
                let _ = self.sock.send_to(&pkt, addr);
            }
        }
    }
}
