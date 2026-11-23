"""
pro3_bridge.py -- Puente completo 2.4 GHz (200 Hz) para 8BitDo Pro 3 en Windows.

Activa la telemetria nativa de 200 Hz en el dongle stock (SIN modificar firmware) y expone:
  1) Servidor DSU / Cemuhook UDP (127.0.0.1:26760) -> Cemu, Ryujinx, Yuzu/Eden, Dolphin, RPCS3, Citra
  2) Mando Virtual Xbox 360 o DualShock 4 (via vgamepad / ViGEmBus, opcional) -> Steam y juegos de PC
  3) Giroscopio para juegos de Steam / PC:
       - --gyro mouse : Gyro -> Mouse (SendInput nativo 200 Hz, cero drivers extra)
       - --gyro stick : Gyro -> Stick Derecho del mando virtual (100% compatible con cualquier juego)
       - --gyro off   : Solo DSU + mando virtual (ideal para emuladores)

Ejemplos:
  python pro3_bridge.py                                # DSU + mando virtual + Gyro->Mouse al mantener L2
  python pro3_bridge.py --gyro off                     # Emuladores (DSU 200 Hz + mando virtual)
  python pro3_bridge.py --gyro mouse --trigger always  # Gyro->Mouse siempre activo (Star/Capture pausa)
  python pro3_bridge.py --gyro stick --trigger lt      # Gyro->Stick Derecho al mantener L2 (Steam)
  python pro3_bridge.py --xinput                       # Volver el dongle a XInput normal (2DC8:310B)
"""
import argparse
import ctypes
import math
import os
import random
import socket
import struct
import sys
import threading
import time
import zlib

import hid

# --- Hardware constants (observed on the stock 2.4 GHz dongle v1.02; cross-checked with SDL's 8BitDo driver) ---
VID_8BITDO = 0x2DC8
PID_XINPUT = 0x310B
PID_DINPUT = 0x600A
PID_IDLE = 0x3109

ACCEL_SCALE = 4096.0                # LSB per 1g
GYRO_DPS_SCALE = 2000.0 / 32767.0   # deg/s per LSB

# 32-bit button bitmask in Report ID 0x02 bytes 18..21 (see docs/PROTOCOL.md, section 4)
BTN_PLUS    = 1 << 0   # Start / Options
BTN_L3      = 1 << 1   # Left Stick Click
BTN_R3      = 1 << 2   # Right Stick Click
BTN_MINUS   = 1 << 3   # Back / Select / Share
BTN_NORTH   = 1 << 4   # Xbox Y / Switch X / PS Triangle
BTN_WEST    = 1 << 5   # Xbox X / Switch Y / PS Square
BTN_DRIGHT  = 1 << 6   # D-Pad Right
BTN_DLEFT   = 1 << 7   # D-Pad Left
BTN_DDOWN   = 1 << 8   # D-Pad Down
BTN_DUP     = 1 << 9   # D-Pad Up
BTN_L1      = 1 << 10  # LB / L1
BTN_R1      = 1 << 11  # RB / R1
BTN_SOUTH   = 1 << 12  # Xbox A / Switch B / PS Cross
BTN_EAST    = 1 << 13  # Xbox B / Switch A / PS Circle
BTN_L2_DIG  = 1 << 14  # LT / L2 digital
BTN_R2_DIG  = 1 << 15  # RT / R2 digital
BTN_CAPTURE = 1 << 16  # Star / Capture
BTN_HOME    = 1 << 17  # Home / Guide / PS


# --- Windows SendInput for smooth 200 Hz relative mouse output ---
if sys.platform == "win32":
    PUL = ctypes.POINTER(ctypes.c_ulong)

    class MOUSEINPUT(ctypes.Structure):
        _fields_ = [
            ("dx", ctypes.c_long),
            ("dy", ctypes.c_long),
            ("mouseData", ctypes.c_ulong),
            ("dwFlags", ctypes.c_ulong),
            ("time", ctypes.c_ulong),
            ("dwExtraInfo", PUL),
        ]

    class _INPUT_UNION(ctypes.Union):
        _fields_ = [("mi", MOUSEINPUT)]

    class INPUT(ctypes.Structure):
        _fields_ = [("type", ctypes.c_ulong), ("union", _INPUT_UNION)]

    _SendInput = ctypes.windll.user32.SendInput
    INPUT_MOUSE = 0
    MOUSEEVENTF_MOVE = 0x0001

    def send_mouse_delta(dx: int, dy: int):
        if dx == 0 and dy == 0:
            return
        inp = INPUT(
            type=INPUT_MOUSE,
            union=_INPUT_UNION(
                mi=MOUSEINPUT(
                    dx=int(dx),
                    dy=int(dy),
                    mouseData=0,
                    dwFlags=MOUSEEVENTF_MOVE,
                    time=0,
                    dwExtraInfo=None,
                )
            ),
        )
        _SendInput(1, ctypes.byref(inp), ctypes.sizeof(INPUT))
else:
    def send_mouse_delta(dx: int, dy: int):
        pass


# --- Dongle connection / mode switching ---
def switch_back_xinput():
    devs = hid.enumerate(VID_8BITDO, PID_DINPUT)
    if not devs:
        if hid.enumerate(VID_8BITDO, PID_XINPUT):
            print("El dongle ya esta en modo XInput (2DC8:310B).")
            return True
        if hid.enumerate(VID_8BITDO, PID_IDLE):
            print("El dongle esta en espera (2DC8:3109). Al encender el mando volvera a XInput.")
            return True
        print("No se encontro el dongle 8BitDo Pro 3.")
        return False
    h = hid.device()
    h.open_path(devs[0]["path"])
    # Clean hardware Watchdog reset (clears RAM telemetry flag without touching flash/bootloader)
    h.write(bytes([0x81, 0x00, 0x1B]) + bytes(61))
    h.close()
    print("Reiniciando dongle al modo XInput de fabrica (~6 s)...")
    deadline = time.time() + 12.0
    saw_idle = False
    while time.time() < deadline:
        if hid.enumerate(VID_8BITDO, PID_XINPUT):
            print("OK: dongle de vuelta en XInput de fabrica (2DC8:310B).")
            return True
        if hid.enumerate(VID_8BITDO, PID_IDLE) and not saw_idle:
            saw_idle = True
            print("Dongle reiniciado (2DC8:3109). Esperando reconexion del mando...")
        time.sleep(0.2)
    if saw_idle:
        print("OK: dongle restaurado (2DC8:3109). Enciende el mando para conectar en XInput (2DC8:310B).")
        return True
    print(
        "Aviso: el dongle tardo mas de lo esperado en reenumerar.\n"
        "       Si no vuelve solo, desconecta y vuelve a conectar el dongle USB."
    )
    return False


def imu_is_live(st) -> bool:
    mag = math.sqrt(st["ax_g"] ** 2 + st["ay_g"] ** 2 + st["az_g"] ** 2)
    return 0.35 <= mag <= 3.0


def drain_hid(h, max_reads=250):
    for _ in range(max_reads):
        if not h.read(64):
            break


def open_dongle_telemetry(wait_for_controller=True):
    """Connects to dongle, enables 200 Hz Report ID 0x02 + live IMU, and returns (hid_device, mac_bytes)."""
    announced_wait = False
    while True:
        devs_600a = hid.enumerate(VID_8BITDO, PID_DINPUT)
        if devs_600a:
            dev_info = devs_600a[0]
            h = hid.device()
            h.open_path(dev_info["path"])
            h.set_nonblocking(True)
            # Check briefly if Report ID 0x02 is streaming WITH live IMU (gravity ~1g)
            t0 = time.time()
            imu_live = False
            while time.time() - t0 < 0.25:
                d = h.read(64)
                if d:
                    st = parse_telemetry(d)
                    if st and imu_is_live(st):
                        imu_live = True
                        break
                else:
                    time.sleep(0.005)
            if imu_live:
                return h, parse_mac_from_serial(dev_info.get("serial_number", ""))
            # Send WIRELESS_ENTER_TEST_MODE_CMD to wake IMU stream on controller
            try:
                h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))
            except Exception:
                h.close()
                time.sleep(0.35)
                continue
            time.sleep(0.35)
            drain_hid(h)
            t0 = time.time()
            while time.time() - t0 < 0.4:
                d = h.read(64)
                if d:
                    st = parse_telemetry(d)
                    if st and imu_is_live(st):
                        return h, parse_mac_from_serial(dev_info.get("serial_number", ""))
                else:
                    time.sleep(0.005)
            h.close()
            time.sleep(0.35)
            continue

        devs_310b = [
            d for d in hid.enumerate(VID_8BITDO, PID_XINPUT)
            if d["usage_page"] == 0xFF7A
        ]
        if devs_310b:
            h = hid.device()
            h.open_path(devs_310b[0]["path"])
            print("Dongle detectado en XInput (2DC8:310B). Activando telemetria 200 Hz (2DC8:600A)...")
            h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))
            h.close()
            # Wait for 2DC8:600A to appear
            deadline = time.time() + 6.0
            while time.time() < deadline:
                if hid.enumerate(VID_8BITDO, PID_DINPUT):
                    time.sleep(0.2)
                    break
                time.sleep(0.05)
            continue

        if not wait_for_controller:
            raise RuntimeError("No se encontro el dongle conectado con el mando encendido.")

        if not announced_wait:
            if hid.enumerate(VID_8BITDO, PID_IDLE):
                print("Dongle en espera (2DC8:3109). Enciende el mando o toca un boton para conectar...")
            else:
                print("Esperando dongle 8BitDo Pro 3 (conecta el dongle y enciende el mando)...")
            announced_wait = True
        time.sleep(0.35)


def parse_mac_from_serial(serial_str: str) -> bytes:
    clean = "".join(c for c in (serial_str or "") if c in "0123456789abcdefABCDEF")
    if len(clean) >= 12:
        try:
            return bytes.fromhex(clean[:12])
        except ValueError:
            pass
    return bytes([0x8B, 0x17, 0xD0, 0x03, 0x24, 0x00])


def send_dongle_rumble(h, large_motor: int, small_motor: int, duration_ms: int = 2000):
    """Sends rumble to 8BitDo Pro 3 via 2DC8:600A CUSTOM_CMD cmd=8 (Report ID 0x81)."""
    lo = max(0, min(255, int(large_motor)))
    hi = max(0, min(255, int(small_motor)))
    dur = 0 if (lo == 0 and hi == 0) else max(50, min(60000, int(duration_ms)))
    pkt = struct.pack("<BBHHBB", 0x81, 0x04, 0x0008, dur, lo, hi) + bytes(56)
    try:
        h.write(pkt)
    except Exception:
        pass


class RumbleState:
    def __init__(self):
        self._lock = threading.Lock()
        self._large = 0
        self._small = 0
        self._dirty = False
        self._last_sent_t = 0.0
        self._last_sent_val = (0, 0)

    def set(self, large_motor: int, small_motor: int):
        lo = max(0, min(255, int(large_motor)))
        hi = max(0, min(255, int(small_motor)))
        with self._lock:
            if lo != self._large or hi != self._small:
                self._large = lo
                self._small = hi
                self._dirty = True

    def set_motor(self, motor_id: int, intensity: int):
        val = max(0, min(255, int(intensity)))
        with self._lock:
            if motor_id == 0:
                if val != self._large:
                    self._large = val
                    self._dirty = True
            elif motor_id == 1:
                if val != self._small:
                    self._small = val
                    self._dirty = True

    def flush(self, h, now: float):
        with self._lock:
            lo, hi = self._large, self._small
            dirty = self._dirty
            self._dirty = False
        # Send immediately on change (min 15ms spacing for rapid updates unless stopping),
        # or refresh every 1.2s while non-zero so the 2.0s dongle safety timer never stops continuous rumble.
        if dirty:
            if (lo == 0 and hi == 0) or (now - self._last_sent_t >= 0.015):
                send_dongle_rumble(h, lo, hi, 2000)
                self._last_sent_t = now
                self._last_sent_val = (lo, hi)
            else:
                with self._lock:
                    self._dirty = True
        elif (lo > 0 or hi > 0) and (now - self._last_sent_t >= 1.2):
            send_dongle_rumble(h, lo, hi, 2000)
            self._last_sent_t = now
            self._last_sent_val = (lo, hi)


def parse_telemetry(pkt):
    if len(pkt) < 42 or pkt[0] != 0x02 or pkt[4] != 0x03 or pkt[6] != 0x18:
        return None
    b = bytes(pkt)
    btns = struct.unpack_from("<I", b, 18)[0]
    lx, ly, rx, ry, r2, l2 = b[22:28]
    ax, ay, az, gx, gy, gz = struct.unpack_from("<hhhhhh", b, 28)
    bat = b[41]
    return {
        "btns": btns,
        "lx": lx,
        "ly": ly,
        "rx": rx,
        "ry": ry,
        "l2": l2,
        "r2": r2,
        "ax_g": ax / ACCEL_SCALE,
        "ay_g": ay / ACCEL_SCALE,
        "az_g": az / ACCEL_SCALE,
        "gx_dps": gx * GYRO_DPS_SCALE,
        "gy_dps": gy * GYRO_DPS_SCALE,
        "gz_dps": gz * GYRO_DPS_SCALE,
        "battery": bat,
    }


# --- DSU / Cemuhook UDP Server (Protocol v1001, 100% spec-compliant) ---
DSU_PROTOCOL_VERSION = 1001
MSG_VERSION = 0x100000
MSG_PORTS   = 0x100001
MSG_DATA    = 0x100002
MSG_MOTORS  = 0x110001
MSG_RUMBLE  = 0x110002


def dsu_battery_code(pct: int) -> int:
    if pct >= 85:
        return 0x05  # Full
    if pct >= 60:
        return 0x04  # High
    if pct >= 35:
        return 0x03  # Medium
    if pct >= 15:
        return 0x02  # Low
    if pct > 0:
        return 0x01  # Dying
    return 0x00


class DSUServer:
    def __init__(self, host="127.0.0.1", port=26760, mac=b"\x8b\x17\xd0\x03\x24\x00", rumble: RumbleState = None):
        self.host = host
        self.port = port
        self.mac = mac
        self.rumble = rumble
        self.server_id = random.randint(1, 0xFFFFFFFF)
        self.sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        self.sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        self.sock.bind((host, port))
        self.sock.setblocking(False)
        self.clients = {}       # addr -> {"expire": float, "pkt_num": int}
        self.connected = False
        self.battery = 80
        self._lock = threading.Lock()
        self._running = True
        self._rx_thread = threading.Thread(target=self._rx_loop, daemon=True)
        self._rx_thread.start()

    def _pack_message(self, msg_type: int, payload: bytes) -> bytes:
        # Length = 4 (msg_type) + len(payload)
        length = 4 + len(payload)
        header_no_crc = struct.pack(
            "<4sHHIII",
            b"DSUS",
            DSU_PROTOCOL_VERSION,
            length,
            0,
            self.server_id,
            msg_type,
        )
        raw = header_no_crc + payload
        crc = zlib.crc32(raw) & 0xFFFFFFFF
        return raw[:8] + struct.pack("<I", crc) + raw[12:]

    def _slot_header(self, slot: int) -> bytes:
        if slot == 0 and self.connected:
            return struct.pack(
                "<BBBB6sB",
                0,                           # slot 0
                2,                           # state: connected
                2,                           # model: Full Gyro (DS4-like)
                1,                           # connection: USB / 2.4G dongle
                self.mac,
                dsu_battery_code(self.battery),
            )
        return struct.pack("<BBBB6sB", slot, 0, 0, 0, b"\x00" * 6, 0)

    def _rx_loop(self):
        while self._running:
            try:
                data, addr = self.sock.recvfrom(1024)
            except BlockingIOError:
                time.sleep(0.005)
                continue
            except OSError:
                break

            if len(data) < 20 or data[:4] != b"DSUC":
                continue

            msg_type = struct.unpack_from("<I", data, 16)[0]
            body = data[20:]

            if msg_type == MSG_VERSION:
                pkt = self._pack_message(MSG_VERSION, struct.pack("<H", DSU_PROTOCOL_VERSION))
                self._safe_send(pkt, addr)

            elif msg_type == MSG_PORTS:
                if len(body) >= 4:
                    count = min(max(struct.unpack_from("<i", body, 0)[0], 0), 4)
                    for i in range(count):
                        if 4 + i < len(body):
                            slot = body[4 + i]
                            payload = self._slot_header(slot) + b"\x00"
                            self._safe_send(self._pack_message(MSG_PORTS, payload), addr)

            elif msg_type == MSG_DATA:
                if len(body) >= 8:
                    flags = body[0]
                    slot = body[1]
                    req_mac = body[2:8]
                    wants_slot0 = (
                        flags == 0
                        or ((flags & 1) and slot == 0)
                        or ((flags & 2) and req_mac == self.mac)
                    )
                    if wants_slot0:
                        with self._lock:
                            entry = self.clients.get(addr)
                            if entry is None:
                                self.clients[addr] = {"expire": time.time() + 5.0, "pkt_num": 0}
                            else:
                                entry["expire"] = time.time() + 5.0

            elif msg_type == MSG_MOTORS:
                motor_cnt = b"\x02" if self.rumble is not None else b"\x00"
                payload = self._slot_header(0) + motor_cnt
                self._safe_send(self._pack_message(MSG_MOTORS, payload), addr)

            elif msg_type == MSG_RUMBLE:
                if self.rumble is not None and len(body) >= 10:
                    motor_id = body[8]
                    intensity = body[9]
                    self.rumble.set_motor(motor_id, intensity)

    def _safe_send(self, pkt: bytes, addr):
        try:
            self.sock.sendto(pkt, addr)
        except OSError:
            pass

    def client_count(self) -> int:
        now = time.time()
        with self._lock:
            expired = [a for a, info in self.clients.items() if info["expire"] < now]
            for a in expired:
                del self.clients[a]
            return len(self.clients)

    def push_state(self, st, gyro_bias, timestamp_us: int):
        self.connected = True
        self.battery = st["battery"]
        now = time.time()
        with self._lock:
            active = [(a, info) for a, info in self.clients.items() if info["expire"] >= now]
            expired = [a for a, info in self.clients.items() if info["expire"] < now]
            for a in expired:
                del self.clients[a]
        if not active:
            return

        btns = st["btns"]
        # DSU Button Byte 1 (offset 16, descending bits 128..1):
        #   0x80=Left, 0x40=Down, 0x20=Right, 0x10=Up, 0x08=Options/Plus, 0x04=R3, 0x02=L3, 0x01=Share/Minus
        b1 = (
            (0x80 if (btns & BTN_DLEFT) else 0)
            | (0x40 if (btns & BTN_DDOWN) else 0)
            | (0x20 if (btns & BTN_DRIGHT) else 0)
            | (0x10 if (btns & BTN_DUP) else 0)
            | (0x08 if (btns & BTN_PLUS) else 0)
            | (0x04 if (btns & BTN_R3) else 0)
            | (0x02 if (btns & BTN_L3) else 0)
            | (0x01 if (btns & BTN_MINUS) else 0)
        )
        # DSU Button Byte 2 (offset 17, descending bits 128..1):
        #   0x80=West/Square, 0x40=South/Cross, 0x20=East/Circle, 0x10=North/Triangle,
        #   0x08=R1, 0x04=L1, 0x02=R2, 0x01=L2
        b2 = (
            (0x80 if (btns & BTN_WEST) else 0)
            | (0x40 if (btns & BTN_SOUTH) else 0)
            | (0x20 if (btns & BTN_EAST) else 0)
            | (0x10 if (btns & BTN_NORTH) else 0)
            | (0x08 if (btns & BTN_R1) else 0)
            | (0x04 if (btns & BTN_L1) else 0)
            | (0x02 if (btns & BTN_R2_DIG or st["r2"] > 20) else 0)
            | (0x01 if (btns & BTN_L2_DIG or st["l2"] > 20) else 0)
        )
        home_b = 1 if (btns & BTN_HOME) else 0
        touch_b = 1 if (btns & BTN_CAPTURE) else 0

        # DSU sticks: X 0=left..255=right, Y 0=down..255=up (invert HID Y)
        lx = st["lx"]
        ly = 255 - st["ly"]
        rx = st["rx"]
        ry = 255 - st["ry"]

        # Axis mapping verified from SDL_hidapi_8bitdo.c + Dolphin DualShockUDPClient.cpp:
        #   Accel: X = +ay (left up), Y = -az (-1g flat on table), Z = +ax (front up)
        #   Gyro : Pitch = -(gy - bias_y), Yaw = -(gz - bias_z), Roll = +(gx - bias_x)
        dsu_ax = st["ay_g"]
        dsu_ay = -st["az_g"]
        dsu_az = st["ax_g"]

        gx = st["gx_dps"] - gyro_bias[0]
        gy = st["gy_dps"] - gyro_bias[1]
        gz = st["gz_dps"] - gyro_bias[2]
        dsu_pitch = -gy
        dsu_yaw = -gz
        dsu_roll = gx

        slot_hdr = self._slot_header(0)
        tail = struct.pack(
            "<BBBBBBBBBBBBBBBBBBBB12sQffffff",
            b1, b2, home_b, touch_b,
            lx, ly, rx, ry,
            255 if (btns & BTN_DLEFT) else 0,
            255 if (btns & BTN_DDOWN) else 0,
            255 if (btns & BTN_DRIGHT) else 0,
            255 if (btns & BTN_DUP) else 0,
            255 if (btns & BTN_WEST) else 0,
            255 if (btns & BTN_SOUTH) else 0,
            255 if (btns & BTN_EAST) else 0,
            255 if (btns & BTN_NORTH) else 0,
            255 if (btns & BTN_R1) else 0,
            255 if (btns & BTN_L1) else 0,
            st["r2"],
            st["l2"],
            b"\x00" * 12,
            timestamp_us,
            dsu_ax, dsu_ay, dsu_az,
            dsu_pitch, dsu_yaw, dsu_roll,
        )

        for addr, info in active:
            pkt_num = info["pkt_num"]
            info["pkt_num"] = (pkt_num + 1) & 0xFFFFFFFF
            payload = slot_hdr + struct.pack("<BI", 1, pkt_num) + tail
            self._safe_send(self._pack_message(MSG_DATA, payload), addr)

    def close(self):
        self._running = False
        try:
            self.sock.close()
        except OSError:
            pass


def _clamp_i16(v: float) -> int:
    iv = int(round(v))
    if iv > 32767:
        return 32767
    if iv < -32768:
        return -32768
    return iv


# --- Optional Virtual Controller via vgamepad (ViGEmBus) ---
class VirtualPad:
    def __init__(self, pad_type="ds4", nintendo_layout=False, rumble: RumbleState = None):
        import vgamepad as vg
        self.vg = vg
        self.pad_type = pad_type
        self.nintendo_layout = nintendo_layout
        self.rumble = rumble
        if pad_type == "ds4":
            self.pad = vg.VDS4Gamepad()
            if rumble is not None:
                def _ds4_cb(client, target, large_motor, small_motor, lightbar_color, user_data):
                    rumble.set(large_motor, small_motor)
                self._cb_ref = _ds4_cb
                self.pad.register_notification(callback_function=_ds4_cb)
        else:
            self.pad = vg.VX360Gamepad()
            if rumble is not None:
                def _x360_cb(client, target, large_motor, small_motor, led_number, user_data):
                    rumble.set(large_motor, small_motor)
                self._cb_ref = _x360_cb
                self.pad.register_notification(callback_function=_x360_cb)

    def update(self, st, gyro_bias=(0.0, 0.0, 0.0), rx_override=None, ry_override=None):
        btns = st["btns"]
        vg = self.vg
        pad = self.pad
        pad.reset()

        lx = (st["lx"] - 127.5) / 127.5
        ly = (st["ly"] - 127.5) / 127.5
        rx = rx_override if rx_override is not None else ((st["rx"] - 127.5) / 127.5)
        ry = ry_override if ry_override is not None else ((st["ry"] - 127.5) / 127.5)
        lx = max(-1.0, min(1.0, lx))
        ly = max(-1.0, min(1.0, ly))
        rx = max(-1.0, min(1.0, rx))
        ry = max(-1.0, min(1.0, ry))

        if self.pad_type == "ds4":
            B = vg.DS4_BUTTONS
            if self.nintendo_layout:
                if btns & BTN_EAST:  pad.press_button(B.DS4_BUTTON_CROSS)
                if btns & BTN_SOUTH: pad.press_button(B.DS4_BUTTON_CIRCLE)
                if btns & BTN_NORTH: pad.press_button(B.DS4_BUTTON_SQUARE)
                if btns & BTN_WEST:  pad.press_button(B.DS4_BUTTON_TRIANGLE)
            else:
                if btns & BTN_SOUTH: pad.press_button(B.DS4_BUTTON_CROSS)
                if btns & BTN_EAST:  pad.press_button(B.DS4_BUTTON_CIRCLE)
                if btns & BTN_WEST:  pad.press_button(B.DS4_BUTTON_SQUARE)
                if btns & BTN_NORTH: pad.press_button(B.DS4_BUTTON_TRIANGLE)
            if btns & BTN_L1:    pad.press_button(B.DS4_BUTTON_SHOULDER_LEFT)
            if btns & BTN_R1:    pad.press_button(B.DS4_BUTTON_SHOULDER_RIGHT)
            if (btns & BTN_L2_DIG) or st["l2"] > 10:
                pad.press_button(B.DS4_BUTTON_TRIGGER_LEFT)
            if (btns & BTN_R2_DIG) or st["r2"] > 10:
                pad.press_button(B.DS4_BUTTON_TRIGGER_RIGHT)
            if btns & BTN_L3:    pad.press_button(B.DS4_BUTTON_THUMB_LEFT)
            if btns & BTN_R3:    pad.press_button(B.DS4_BUTTON_THUMB_RIGHT)
            if btns & BTN_MINUS: pad.press_button(B.DS4_BUTTON_SHARE)
            if btns & BTN_PLUS:  pad.press_button(B.DS4_BUTTON_OPTIONS)
            if btns & BTN_HOME:  pad.press_special_button(vg.DS4_SPECIAL_BUTTONS.DS4_SPECIAL_BUTTON_PS)
            if btns & BTN_CAPTURE: pad.press_special_button(vg.DS4_SPECIAL_BUTTONS.DS4_SPECIAL_BUTTON_TOUCHPAD)

            # D-Pad hat
            up = bool(btns & BTN_DUP)
            dn = bool(btns & BTN_DDOWN)
            lf = bool(btns & BTN_DLEFT)
            rt = bool(btns & BTN_DRIGHT)
            D = vg.DS4_DPAD_DIRECTIONS
            if up and rt:   pad.directional_pad(D.DS4_BUTTON_DPAD_NORTHEAST)
            elif up and lf: pad.directional_pad(D.DS4_BUTTON_DPAD_NORTHWEST)
            elif dn and rt: pad.directional_pad(D.DS4_BUTTON_DPAD_SOUTHEAST)
            elif dn and lf: pad.directional_pad(D.DS4_BUTTON_DPAD_SOUTHWEST)
            elif up:        pad.directional_pad(D.DS4_BUTTON_DPAD_NORTH)
            elif dn:        pad.directional_pad(D.DS4_BUTTON_DPAD_SOUTH)
            elif lf:        pad.directional_pad(D.DS4_BUTTON_DPAD_WEST)
            elif rt:        pad.directional_pad(D.DS4_BUTTON_DPAD_EAST)
            else:           pad.directional_pad(D.DS4_BUTTON_DPAD_NONE)

            pad.left_joystick(st["lx"], st["ly"])
            if rx_override is None and ry_override is None:
                pad.right_joystick(st["rx"], st["ry"])
            else:
                pad.right_joystick_float(x_value_float=rx, y_value_float=ry)
            pad.left_trigger(value=st["l2"])
            pad.right_trigger(value=st["r2"])

            # Feed REAL 6-axis Gyro + Accel into DS4_REPORT_EX so Steam Input sees native DS4 motion!
            # Exact SDL/DS4 right-handed coordinate frame (verified from SDL_hidapi_8bitdo.c & SDL_hidapi_ps4.c):
            #   Gyro (16.3835 LSB / deg/s): X=Pitch(-gy), Y=Yaw(+gz), Z=Roll(-gx)
            #   Accel (8192 LSB / 1g):      X=Right(-ay), Y=Up(+az),  Z=Back(-ax)
            gx_dps = st["gx_dps"] - gyro_bias[0]
            gy_dps = st["gy_dps"] - gyro_bias[1]
            gz_dps = st["gz_dps"] - gyro_bias[2]
            ds4_gyro_scale = 32767.0 / 2000.0
            pad.gyroscope(
                _clamp_i16(-gy_dps * ds4_gyro_scale),
                _clamp_i16(+gz_dps * ds4_gyro_scale),
                _clamp_i16(-gx_dps * ds4_gyro_scale),
            )
            pad.accelerometer(
                _clamp_i16(-st["ay_g"] * 8192.0),
                _clamp_i16(+st["az_g"] * 8192.0),
                _clamp_i16(-st["ax_g"] * 8192.0),
            )
            pad.usb(True)
            pad.battery(min(11, max(0, st["battery"] // 10)))
        else:
            B = vg.XUSB_BUTTON
            if self.nintendo_layout:
                if btns & BTN_EAST:  pad.press_button(B.XUSB_GAMEPAD_A)
                if btns & BTN_SOUTH: pad.press_button(B.XUSB_GAMEPAD_B)
                if btns & BTN_NORTH: pad.press_button(B.XUSB_GAMEPAD_X)
                if btns & BTN_WEST:  pad.press_button(B.XUSB_GAMEPAD_Y)
            else:
                if btns & BTN_SOUTH: pad.press_button(B.XUSB_GAMEPAD_A)
                if btns & BTN_EAST:  pad.press_button(B.XUSB_GAMEPAD_B)
                if btns & BTN_WEST:  pad.press_button(B.XUSB_GAMEPAD_X)
                if btns & BTN_NORTH: pad.press_button(B.XUSB_GAMEPAD_Y)
            if btns & BTN_DUP:    pad.press_button(B.XUSB_GAMEPAD_DPAD_UP)
            if btns & BTN_DDOWN:  pad.press_button(B.XUSB_GAMEPAD_DPAD_DOWN)
            if btns & BTN_DLEFT:  pad.press_button(B.XUSB_GAMEPAD_DPAD_LEFT)
            if btns & BTN_DRIGHT: pad.press_button(B.XUSB_GAMEPAD_DPAD_RIGHT)
            if btns & BTN_L1:     pad.press_button(B.XUSB_GAMEPAD_LEFT_SHOULDER)
            if btns & BTN_R1:     pad.press_button(B.XUSB_GAMEPAD_RIGHT_SHOULDER)
            if btns & BTN_L3:     pad.press_button(B.XUSB_GAMEPAD_LEFT_THUMB)
            if btns & BTN_R3:     pad.press_button(B.XUSB_GAMEPAD_RIGHT_THUMB)
            if btns & BTN_MINUS:  pad.press_button(B.XUSB_GAMEPAD_BACK)
            if btns & BTN_PLUS:   pad.press_button(B.XUSB_GAMEPAD_START)
            if btns & BTN_HOME:   pad.press_button(B.XUSB_GAMEPAD_GUIDE)

            # X360 Y axis is +1.0 up, -1.0 down (invert HID Y)
            pad.left_joystick_float(x_value_float=lx, y_value_float=-ly)
            pad.right_joystick_float(x_value_float=rx, y_value_float=-ry)
            pad.left_trigger(value=st["l2"])
            pad.right_trigger(value=st["r2"])

        pad.update()

    def close(self):
        try:
            self.pad.reset()
            self.pad.update()
        except Exception:
            pass


# --- Gyro calibration & aiming processor ---
def calibrate_gyro(h, duration=0.5):
    print(f"Calibrando giroscopio ({duration:.1f}s)... deja el mando quieto un momento.")
    xs, ys, zs = [], [], []
    t0 = time.time()
    while time.time() - t0 < duration:
        d = h.read(64)
        if d:
            st = parse_telemetry(d)
            if st and imu_is_live(st):
                xs.append(st["gx_dps"])
                ys.append(st["gy_dps"])
                zs.append(st["gz_dps"])
        else:
            time.sleep(0.001)
    if not xs:
        return [0.0, 0.0, 0.0]
    span = max(max(xs) - min(xs), max(ys) - min(ys), max(zs) - min(zs))
    if span > 12.0:
        print(f"Aviso: el mando se movio durante la calibracion (rango {span:.1f} deg/s). Usando offset 0.0 (manten Minus+Plus 0.5s quieto para recalibrar).")
        return [0.0, 0.0, 0.0]
    bias = [sum(xs) / len(xs), sum(ys) / len(ys), sum(zs) / len(zs)]
    print(f"Calibracion lista ({len(xs)} muestras): offset=({bias[0]:+.2f}, {bias[1]:+.2f}, {bias[2]:+.2f}) deg/s")
    return bias


def is_gyro_active(st, trigger_mode: str, toggle_state: bool) -> bool:
    btns = st["btns"]
    if trigger_mode == "always":
        # Hold Capture/Star to pause/ratchet gyro (like lifting a mouse)
        return not bool(btns & BTN_CAPTURE)
    if trigger_mode == "toggle":
        return toggle_state
    if trigger_mode == "lt":
        return st["l2"] >= 45 or bool(btns & BTN_L2_DIG)
    if trigger_mode == "rt":
        return st["r2"] >= 45 or bool(btns & BTN_R2_DIG)
    if trigger_mode == "lb":
        return bool(btns & BTN_L1)
    if trigger_mode == "rb":
        return bool(btns & BTN_R1)
    return False


def main():
    ap = argparse.ArgumentParser(description="8BitDo Pro 3 2.4 GHz 200Hz Gyro Bridge (DSU + Steam)")
    ap.add_argument("--xinput", action="store_true", help="Volver el dongle a XInput (2DC8:310B) y salir")
    ap.add_argument("--restore-xinput", action="store_true", help="Al cerrar con Ctrl+C, volver el dongle a XInput")
    ap.add_argument("--no-dsu", action="store_true", help="Desactivar el servidor DSU/Cemuhook UDP")
    ap.add_argument("--dsu-port", type=int, default=26760, help="Puerto UDP DSU (default: 26760)")
    ap.add_argument("--pad", choices=["auto", "ds4", "xbox", "none"], default="auto",
                    help="Mando virtual ViGEmBus (auto=ds4 con gyro nativo para Steam, ds4, xbox, none)")
    ap.add_argument("--layout", choices=["xbox", "nintendo"], default="xbox",
                    help="Mapa de botones A/B/X/Y para el mando virtual (default: xbox)")
    ap.add_argument("--gyro", choices=["auto", "mouse", "stick", "off"], default="auto",
                    help="Modo de gyro extra: auto (off en ds4 porque Steam lee gyro nativo; mouse en xbox), mouse, stick, off")
    ap.add_argument("--trigger", choices=["lt", "rt", "lb", "rb", "always", "toggle"], default="lt",
                    help="Cuando se activa el gyro en PC/Steam (default: lt = mantener gatillo izquierdo)")
    ap.add_argument("--sens", type=float, default=1.4, help="Sensibilidad gyro (default: 1.4)")
    ap.add_argument("--roll-mix", type=float, default=0.35,
                    help="Mezcla de Roll en eje horizontal (0.0=solo Yaw, 0.35=natural)")
    ap.add_argument("--deadzone", type=float, default=0.8, help="Zona muerta gyro en deg/s (default: 0.8)")
    ap.add_argument("--invert-x", action="store_true", help="Invertir eje X de apuntado gyro")
    ap.add_argument("--invert-y", action="store_true", help="Invertir eje Y de apuntado gyro")
    ap.add_argument("--no-cal", action="store_true", help="Omitir calibracion inicial de gyro")
    args = ap.parse_args()

    if args.xinput:
        switch_back_xinput()
        return

    rumble = RumbleState()

    # 1. Start DSU server
    dsu = None
    if not args.no_dsu:
        try:
            dsu = DSUServer(port=args.dsu_port, rumble=rumble)
            print(f"[DSU] Servidor Cemuhook activo en 127.0.0.1:{args.dsu_port} (Cemu/Ryujinx/Yuzu/Dolphin/RPCS3)")
        except OSError as e:
            print(f"[DSU] Aviso: no se pudo abrir puerto UDP {args.dsu_port} ({e}). Continuando sin DSU.")

    # 2. Optional Virtual Gamepad (ViGEmBus)
    vpad = None
    if args.pad != "none":
        pad_kind = "ds4" if args.pad == "auto" else args.pad
        try:
            vpad = VirtualPad(pad_type=pad_kind, nintendo_layout=(args.layout == "nintendo"), rumble=rumble)
            if pad_kind == "ds4":
                print("[PAD] Mando virtual DUALSHOCK 4 con GYRO NATIVO + VIBRACION activo (ViGEmBus)!")
            else:
                print(f"[PAD] Mando virtual {pad_kind.upper()} + VIBRACION activo (ViGEmBus).")
        except ImportError:
            print("[PAD] vgamepad no esta instalado (solo DSU + Gyro->Mouse activos).")
            print("      Para activar mando virtual en Steam/PC: pip install vgamepad")
        except Exception as e:
            print(f"[PAD] ViGEmBus no disponible ({e}).")
            print("      Instala ViGEmBus (o ejecuta: pip install vgamepad) para mando virtual en Steam.")

    if args.gyro == "auto":
        args.gyro = "off" if (vpad and vpad.pad_type == "ds4") else "mouse"

    if args.gyro == "stick" and vpad is None:
        print("[AVISO] --gyro stick requiere mando virtual (vgamepad + ViGEmBus). Cambiando a --gyro mouse.")
        args.gyro = "mouse"

    # 3. Open dongle in 200 Hz telemetry mode
    h, mac = open_dongle_telemetry(wait_for_controller=True)
    if dsu:
        dsu.mac = mac
    gyro_bias = [0.0, 0.0, 0.0] if args.no_cal else calibrate_gyro(h, 0.5)

    print("\n--- 8BitDo Pro 3 2.4 GHz Bridge ACTIVO ---")
    print(f"  Modo Gyro PC : {args.gyro} | Activacion: {args.trigger} | Sensibilidad: {args.sens}")
    print("  Recalibrar   : manten Minus + Plus (Select + Start) 0.5s")
    print("  Salir        : Ctrl+C\n")

    # Runtime state
    acc_mx = 0.0
    acc_my = 0.0
    toggle_state = True
    prev_capture = False
    recal_hold_start = None
    last_pkt_t = time.perf_counter()
    last_status_t = time.time()
    pkt_count = 0
    dead_imu_count = 0
    hz_window_start = time.time()
    hz_est = 0.0
    t_epoch = time.perf_counter()

    # Switch to event-driven blocking reads (5ms timeout) for lowest possible latency (~0.2ms)
    h.set_nonblocking(False)
    try:
        while True:
            try:
                d = h.read(64, 5)
            except OSError:
                print("\n[!] Conexion perdida con el dongle. Reintentando...")
                try:
                    h.close()
                except Exception:
                    pass
                if dsu:
                    dsu.connected = False
                h, mac = open_dongle_telemetry(wait_for_controller=True)
                h.set_nonblocking(False)
                if dsu:
                    dsu.mac = mac
                last_pkt_t = time.perf_counter()
                continue

            if not d:
                continue

            st = parse_telemetry(d)
            if not st:
                continue

            # Auto-heal if controller slept/reconnected and stopped sending IMU
            if not imu_is_live(st):
                dead_imu_count += 1
                if dead_imu_count == 60:
                    try:
                        h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))
                    except Exception:
                        pass
                    dead_imu_count = 0
            else:
                dead_imu_count = 0

            now_perf = time.perf_counter()
            dt = min(max(now_perf - last_pkt_t, 0.001), 0.05)
            last_pkt_t = now_perf
            pkt_count += 1

            # Handle toggle button (Capture/Star) on rising edge
            cap = bool(st["btns"] & BTN_CAPTURE)
            if cap and not prev_capture and args.trigger == "toggle":
                toggle_state = not toggle_state
            prev_capture = cap

            # Hold Minus + Plus for 0.5s to recalibrate gyro live
            if (st["btns"] & BTN_MINUS) and (st["btns"] & BTN_PLUS):
                if recal_hold_start is None:
                    recal_hold_start = now_perf
                elif now_perf - recal_hold_start >= 0.5:
                    gyro_bias = calibrate_gyro(h, 0.4)
                    recal_hold_start = None
                    last_pkt_t = time.perf_counter()
                    continue
            else:
                recal_hold_start = None

            # 1) Feed DSU / Cemuhook server
            if dsu:
                ts_us = int((now_perf - t_epoch) * 1_000_000)
                dsu.push_state(st, gyro_bias, ts_us)

            # Compute bias-corrected angular rates (deg/s)
            gx = st["gx_dps"] - gyro_bias[0]
            gy = st["gy_dps"] - gyro_bias[1]
            gz = st["gz_dps"] - gyro_bias[2]

            # Natural aiming axes:
            #   yaw_right  = -gz  (turn controller right = +)
            #   roll_right = +gx  (tilt right = +)
            #   pitch_up   = -gy  (tilt nose up = +)
            horiz_dps = (-gz) + args.roll_mix * gx
            vert_dps = -gy
            if abs(horiz_dps) < args.deadzone:
                horiz_dps = 0.0
            if abs(vert_dps) < args.deadzone:
                vert_dps = 0.0
            if args.invert_x:
                horiz_dps = -horiz_dps
            if args.invert_y:
                vert_dps = -vert_dps

            gyro_on = (args.gyro != "off") and is_gyro_active(st, args.trigger, toggle_state)

            # 2) Gyro -> Mouse (native Windows SendInput)
            rx_ov = ry_ov = None
            if gyro_on and args.gyro == "mouse":
                # Scale: ~12 counts/deg at sens=1.0 -> 360 deg = ~4320 counts
                scale = args.sens * 12.0 * dt
                acc_mx += horiz_dps * scale
                acc_my += (-vert_dps) * scale   # screen Y is downward
                dx = int(math.trunc(acc_mx))
                dy = int(math.trunc(acc_my))
                if dx or dy:
                    acc_mx -= dx
                    acc_my -= dy
                    send_mouse_delta(dx, dy)
            elif gyro_on and args.gyro == "stick":
                # Blend gyro velocity into right stick (-1..+1, HID Y down = +)
                base_rx = (st["rx"] - 127.5) / 127.5
                base_ry = (st["ry"] - 127.5) / 127.5
                # At sens=1.0, 120 deg/s = full stick deflection
                stick_gain = args.sens / 120.0
                rx_ov = max(-1.0, min(1.0, base_rx + horiz_dps * stick_gain))
                ry_ov = max(-1.0, min(1.0, base_ry + (-vert_dps) * stick_gain))
            else:
                acc_mx = 0.0
                acc_my = 0.0

            # 3) Feed Virtual Controller (if ViGEmBus is active)
            if vpad:
                vpad.update(st, gyro_bias=gyro_bias, rx_override=rx_ov, ry_override=ry_ov)

            # 4) Flush any pending rumble commands from Steam / ViGEmBus / DSU to the dongle
            rumble.flush(h, now_perf)

            # Periodic status line (4 Hz)
            now_wall = time.time()
            if now_wall - last_status_t >= 0.25:
                dt_win = now_wall - hz_window_start
                if dt_win >= 1.0:
                    hz_est = pkt_count / dt_win
                    pkt_count = 0
                    hz_window_start = now_wall
                dsu_n = dsu.client_count() if dsu else 0
                pad_s = vpad.pad_type.upper() if vpad else "OFF"
                g_s = f"ON({args.gyro})" if gyro_on else ("OFF" if args.gyro == "off" else "wait")
                sys.stdout.write(
                    f"\r  {hz_est:5.1f} Hz | Bat:{st['battery']:3d}% | DSU:{dsu_n} cli | "
                    f"Pad:{pad_s:4s} | Gyro:{g_s:11s} | L2:{st['l2']:3d} R2:{st['r2']:3d}   "
                )
                sys.stdout.flush()
                last_status_t = now_wall

    except KeyboardInterrupt:
        print("\nDeteniendo puente...")
    finally:
        try:
            send_dongle_rumble(h, 0, 0, 0)
            h.close()
        except Exception:
            pass
        if vpad:
            vpad.close()
        if dsu:
            dsu.close()
        if args.restore_xinput:
            switch_back_xinput()


if __name__ == "__main__":
    main()
