# 8BitDo Pro 3 · 2.4 GHz Dongle HID Protocol

This document describes the USB HID protocol that `pro3-bridge` uses to talk to the
**8BitDo Pro 3** through its stock **2.4 GHz USB dongle**. Every constant, offset and
conversion below matches the reference implementation:

| Topic | Source of truth |
|---|---|
| USB IDs, enable command, telemetry parser, rumble | [`pro3-bridge/src/protocol.rs`](../pro3-bridge/src/protocol.rs) |
| Cemuhook DSU server | [`pro3-bridge/src/dsu.rs`](../pro3-bridge/src/dsu.rs) |
| Virtual DualShock 4 / Xbox 360 (ViGEmBus) | [`pro3-bridge/src/vigem.rs`](../pro3-bridge/src/vigem.rs) |
| Rumble scheduling, paddle handling | [`pro3-bridge/src/bridge.rs`](../pro3-bridge/src/bridge.rs) |

All multi-byte fields are **little-endian**. Offsets are **0-based** and **include the
Report ID byte** (that is, `pkt[0]` is the Report ID, exactly as `hidapi` returns it).

> [!WARNING]
> This protocol is unofficial and was worked out by observation. It is not endorsed by
> 8BitDo. Everything documented here is **RAM-only**: nothing is written to the dongle's
> or the controller's flash. **Do not send firmware-update / bootloader opcodes**, such as
> Report ID `0x81` with the `0x51 0x00` or `0x51 0x05` sequences. These are part of the
> vendor's update path, and sending them can leave the dongle unusable until it is
> reflashed with the official tool. `pro3-bridge` never sends them.

---

## 1. USB identities

The dongle uses vendor ID `0x2DC8` (8BitDo) and shows up as one of three product IDs,
depending on its state:

| VID:PID | Constant | State | Notes |
|---|---|---|---|
| `2DC8:310B` | `PID_XINPUT` | **Stock XInput mode** (default after plug-in) | Composite device. The vendor command channel is the HID interface `MI_02`, **Usage Page `0xFF7A`**. Open that interface specifically (`usage_page() == 0xFF7A`). |
| `2DC8:600A` | `PID_DINPUT` | **RAM telemetry mode** | Entered with the enable command (§2). Streams Report ID `0x02` at about **200 Hz** and accepts rumble (§5). |
| `2DC8:3109` | `PID_IDLE` | **Idle receiver** | Dongle is powered but no controller is paired or awake. Wait for the controller. |

### Connection state machine (`try_connect_dongle`)

1. **`600A` present**: open it and read for up to 220 ms. If a packet parses and its IMU is
   live (§3.4), you are connected. If not, send the enable command again, wait 320 ms,
   drain stale packets and read for another 350 ms.
2. **`310B` + Usage Page `0xFF7A` present**: send the enable command, close the handle
   and wait about 400 ms. The dongle re-enumerates as `600A`. Then go back to step 1.
3. **`3109` present**: wait for the controller.
4. **Nothing found**: the dongle is not plugged in.

The DSU MAC address is taken from the HID serial string (the first 12 hex digits). If the
serial is missing or malformed, it falls back to `8B:17:D0:03:24:00`.

---

## 2. Enabling 200 Hz IMU telemetry

Write one 64-byte output report on the `0xFF7A` interface (in `310B` mode) or on the
`600A` device. Sending it again in `600A` mode wakes an idle IMU stream.

```
Offset  0     1     2     3     4 … 63
Value   0x81  0x06  0x08  0x00  0x00 …
        │     └─ WIRELESS_ENTER_TEST_MODE_CMD (0x06, 0x08, 0x00)
        └─ Report ID 0x81 (vendor command)
```

```rust
let mut pkt = [0u8; 64];
pkt[..4].copy_from_slice(&[0x81, 0x06, 0x08, 0x00]);
dev.write(&pkt)?;
```

The command:

- is **RAM-only**: it changes the dongle's runtime state and nothing is persisted.
- makes the dongle re-enumerate as `2DC8:600A` within a few hundred milliseconds.
- is **undone by unplugging and replugging the dongle**, which brings back stock
  `2DC8:310B` XInput. This is the supported way to revert.

> The Python reference (`python/pro3_bridge.py --xinput`) also tries to switch back over
> software with `[0x81, 0x00, 0x51, 0x02, …]`. Once telemetry mode is active, the firmware
> usually ignores this request. Treat the physical replug as the real revert path, and do
> not try other `0x51 xx` values (see the warning above).

---

## 3. Telemetry input report (Report ID `0x02`)

The report is 64 bytes and arrives at about 200 Hz in `600A` mode. A parser needs **at
least 42 bytes**.

### 3.1 Header signature

A packet is valid only when **all** of these hold:

| Offset | Value |
|---|---|
| `pkt[0]` | `0x02` (Report ID) |
| `pkt[4]` | `0x03` |
| `pkt[6]` | `0x18` |

Drop any other packet.

### 3.2 Layout

| Offset | Size | Type | Field | Scale / range |
|---|---|---|---|---|
| `0` | 1 | `u8` | Report ID | `0x02` |
| `1..3` | 3 | | not interpreted | |
| `4` | 1 | `u8` | signature | `0x03` |
| `5` | 1 | | not interpreted | |
| `6` | 1 | `u8` | signature | `0x18` |
| `7..17` | 11 | | not interpreted | |
| `18..21` | 4 | `u32 LE` | **button bitmask** | see §4 |
| `22` | 1 | `u8` | **LX** | `0..255`, centre ≈ `0x80`, `0` = left |
| `23` | 1 | `u8` | **LY** | `0..255`, centre ≈ `0x80`, `0` = **up** (HID convention) |
| `24` | 1 | `u8` | **RX** | same as LX |
| `25` | 1 | `u8` | **RY** | same as LY |
| `26` | 1 | `u8` | **R2** (analog) | `0..255` |
| `27` | 1 | `u8` | **L2** (analog) | `0..255` |
| `28..29` | 2 | `i16 LE` | **accel X** | `4096 LSB = 1 g` |
| `30..31` | 2 | `i16 LE` | **accel Y** | `4096 LSB = 1 g` |
| `32..33` | 2 | `i16 LE` | **accel Z** | `4096 LSB = 1 g` |
| `34..35` | 2 | `i16 LE` | **gyro X** | `±32767 = ±2000 °/s` |
| `36..37` | 2 | `i16 LE` | **gyro Y** | `±32767 = ±2000 °/s` |
| `38..39` | 2 | `i16 LE` | **gyro Z** | `±32767 = ±2000 °/s` |
| `40` | 1 | | not interpreted | |
| `41` | 1 | `u8` | **battery** | percent, `0..100` |
| `42..63` | 22 | | not interpreted | |

> Note the trigger order: **byte 26 is R2 and byte 27 is L2**.

### 3.3 Scaling

```rust
pub const ACCEL_SCALE: f32 = 4096.0;            // LSB per g
pub const GYRO_DPS_SCALE: f32 = 2000.0 / 32767.0; // °/s per LSB (≈ 0.061037)

accel_g = raw_i16 as f32 / ACCEL_SCALE;
gyro_dps = raw_i16 as f32 * GYRO_DPS_SCALE;
```

### 3.4 Liveness check

When the controller's IMU is asleep, the dongle can keep sending packets with the IMU
fields zeroed. A packet counts as **live** when the accelerometer magnitude is plausible:

```
0.35 g ≤ sqrt(ax² + ay² + az²) ≤ 3.0 g
```

### 3.5 Gyro bias calibration

`calibrate_gyro` collects live samples for a fixed window and averages each gyro axis to
get the bias. The calibration is **rejected** (bias stays at `[0, 0, 0]`) when no live
samples arrive, or when the min-to-max spread of any axis exceeds **12 °/s**, which
means the controller moved. Subtract the bias from `gx/gy/gz` (in °/s) before any frame
conversion.

---

## 4. Button bitmask (`pkt[18..21]`, `u32 LE`)

| Bit | Mask | Constant | Button |
|---:|---|---|---|
| 0 | `0x0000_0001` | `BTN_PLUS` | Plus / Start / Options |
| 1 | `0x0000_0002` | `BTN_L3` | Left stick click |
| 2 | `0x0000_0004` | `BTN_R3` | Right stick click |
| 3 | `0x0000_0008` | `BTN_MINUS` | Minus / Select / Share |
| 4 | `0x0000_0010` | `BTN_NORTH` | North face (Xbox Y · Switch X · PS △) |
| 5 | `0x0000_0020` | `BTN_WEST` | West face (Xbox X · Switch Y · PS □) |
| 6 | `0x0000_0040` | `BTN_DRIGHT` | D-pad right |
| 7 | `0x0000_0080` | `BTN_DLEFT` | D-pad left |
| 8 | `0x0000_0100` | `BTN_DDOWN` | D-pad down |
| 9 | `0x0000_0200` | `BTN_DUP` | D-pad up |
| 10 | `0x0000_0400` | `BTN_L1` | L1 / LB |
| 11 | `0x0000_0800` | `BTN_R1` | R1 / RB |
| 12 | `0x0000_1000` | `BTN_SOUTH` | South face (Xbox A · Switch B · PS ✕) |
| 13 | `0x0000_2000` | `BTN_EAST` | East face (Xbox B · Switch A · PS ○) |
| 14 | `0x0000_4000` | `BTN_L2_DIG` | L2 digital |
| 15 | `0x0000_8000` | `BTN_R2_DIG` | R2 digital |
| 16 | `0x0001_0000` | `BTN_CAPTURE` | Star / Capture |
| 17 | `0x0002_0000` | `BTN_HOME` | Home / Guide |
| 21 | `0x0020_0000` | `BTN_P3` | Extra key-map slot (`KEY_P3_MAP`) |
| **25** | `0x0200_0000` | `BTN_P1` | **Back paddle PL** (independent) |
| **26** | `0x0400_0000` | `BTN_P2` | **Back paddle PR** (independent) |
| 30 | `0x4000_0000` | `BTN_P4` | Extra key-map slot (`KEY_P4_MAP`) |

Bits 18–20, 22–24, 27–29 and 31 are not used by the bridge.

The face buttons are reported by **position** (South/East/West/North), not by label. The
bridge's *Nintendo* layout option swaps South↔East and West↔North when it builds the
virtual pad.

### Back paddles and L4 / R4

- **PL (bit 25)** and **PR (bit 26)** come over the 2.4 GHz link as their own raw bits
  and can be remapped freely on the host.
- The dongle's `0xA2` radio packet does **not** carry **L4 / R4** as separate raw bits
  when they are unmapped. To use them, map them to a standard button with the
  controller's on-board profile or with **8BitDo Ultimate Software**. They then show up
  as that standard button's bit.
- Bits 21 and 30 are firmware key-map slots. The bridge can bind actions to them (paddle
  slots P3/P4), but unmapped L4/R4 do not set them.

---

## 5. Rumble output report (Report ID `0x81`, `600A` only)

```
Offset  0     1     2     3     4       5       6            7            8 … 63
Value   0x81  0x04  0x08  0x00  dur_lo  dur_hi  large_motor  small_motor  0x00 …
        │     │     └─ cmd 0x08 = rumble
        │     └─ CUSTOM_CMD
        └─ Report ID 0x81
```

| Field | Type | Meaning |
|---|---|---|
| `dur` (`4..5`) | `u16 LE` | Motor-on time in **milliseconds**. The bridge clamps it to `50..60000`. |
| `large_motor` (`6`) | `u8` | Left / low-frequency motor, `0..255` |
| `small_motor` (`7`) | `u8` | Right / high-frequency motor, `0..255` |

- **Stop**: send `dur = 0`, or `large = small = 0`. The motors stop at once. When both
  intensities are `0`, `send_dongle_rumble` forces `dur = 0`.
- **Auto-stop safety timer**: the motors stop on their own when `dur` runs out. The bridge
  sends `dur = 2000 ms` and **refreshes** non-zero rumble every 1200 ms. If the host
  crashes or the USB link drops, vibration stops within 2 s instead of running forever.
- **Rate limiting** (`bridge.rs`): a changed intensity is sent at most every 15 ms. A
  transition to `(0, 0)` is always sent at once. A final stop (`0, 0, dur = 0`) is sent
  on disconnect or pause.
- The **Rumble strength** setting multiplies both intensities by `0.1..1.0` before they
  are sent.

Rumble sources in priority order: the UI test pulse (320 ms), then the latest values
from a DSU client or the ViGEm target notification.

---

## 6. Sensor frame conversions

Raw axes are named `ax, ay, az` (g) and `gx, gy, gz` (°/s, bias already subtracted) as
parsed in §3. The mappings below are exactly what the bridge emits. They match the
**SDL / DualShock 4** right-handed sensor frame that Steam Input and the emulators
expect.

### 6.1 Virtual DualShock 4 (`DS4_REPORT_EX` via ViGEmBus)

| DS4 field | Formula | Scale |
|---|---|---|
| `gyro_x` (pitch) | `-gy × 16.3835` | `32767 / 2000 = 16.3835 LSB / (°/s)` |
| `gyro_y` (yaw) | `+gz × 16.3835` | |
| `gyro_z` (roll) | `-gx × 16.3835` | |
| `accel_x` | `-ay × 8192` | `8192 LSB / g` |
| `accel_y` | `+az × 8192` | |
| `accel_z` | `-ax × 8192` | |

All values are rounded and clamped to `i16`. Other DS4 fields:

| DS4 field | Value |
|---|---|
| `thumb_lx/ly/rx/ry` | Raw `u8` sticks passed through unchanged (DS4 also uses `0` = up) |
| `trigger_left / trigger_right` | `L2` (byte 27) / `R2` (byte 26) |
| `TRIGGER_LEFT/RIGHT` button flag | Digital bit **or** analog value `> 10` |
| `SHARE` / `OPTIONS` | Minus / Plus |
| `PS` | Home |
| `TOUCHPAD` click | Capture, or a paddle bound to *Touchpad Click* |
| `timestamp` | `elapsed_µs × 3 / 16` (5.33 µs ticks, 187 500 Hz), wraps at `u16` |
| `battery_lvl` | Battery percent |
| `battery_lvl_special` | `0x10 \| min(battery / 10, 11)` (flagged as USB-powered) |

### 6.2 Virtual Xbox 360 (no motion)

Sticks become signed `i16` through `((v − 127.5) / 127.5) × 32767`. **Y axes are
inverted** (XInput uses `+Y` = up). Triggers pass through as `u8`.

### 6.3 Cemuhook DSU (protocol v1001, UDP `127.0.0.1:26760`)

DSU uses `f32` values in **g** and **°/s**:

| DSU field | Formula |
|---|---|
| `accel_x` | `+ay` |
| `accel_y` | `-az` |
| `accel_z` | `+ax` |
| `gyro_pitch` | `-gy` |
| `gyro_yaw` | `-gz` |
| `gyro_roll` | `+gx` |

Server details as implemented in `dsu.rs`:

- **Header**: `"DSUS"`, `u16` version `1001`, `u16` length (payload + 4), `u32` CRC32
  (computed with the CRC field zeroed), `u32` server ID, `u32` message type.
- **Message types**: `0x100000` version, `0x100001` port info, `0x100002` pad data,
  `0x110001` motor count (reports **2** motors), `0x110002` rumble (`motor 0` = large,
  `motor 1` = small).
- **Slot 0 header**: state `2` (connected), model `2` (full gyro), connection `1` (USB),
  6-byte MAC, battery code (`≥85% → 0x05`, `≥60% → 0x04`, `≥35% → 0x03`,
  `≥15% → 0x02`, `>0% → 0x01`, else `0x00`).
- **Sticks**: X passes through. **Y is inverted** (`255 − raw`) because DSU uses `+Y` = up.
- **Analog buttons**: D-pad and face/shoulder buttons are sent as `0` / `255`. `R2` and
  `L2` are sent as their analog values. The digital L2/R2 bit is set when the digital
  bit is pressed or the analog value is `> 20`.
- **Timestamp**: `u64` microseconds.
- **Subscriptions**: a client is dropped after **5 s** without a fresh data request.

---

## 7. Minimal reader (pseudo-code)

```python
import hid, struct

VID, PID_TELEMETRY = 0x2DC8, 0x600A
h = hid.device(); h.open(VID, PID_TELEMETRY)
h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))  # (re)enable IMU stream

while True:
    p = bytes(h.read(64, 20))
    if len(p) < 42 or p[0] != 0x02 or p[4] != 0x03 or p[6] != 0x18:
        continue
    btns = struct.unpack_from("<I", p, 18)[0]
    lx, ly, rx, ry, r2, l2 = p[22:28]
    ax, ay, az, gx, gy, gz = struct.unpack_from("<6h", p, 28)
    accel_g  = (ax / 4096, ay / 4096, az / 4096)
    gyro_dps = (gx * 2000 / 32767, gy * 2000 / 32767, gz * 2000 / 32767)
    battery  = p[41]
```

The full reference implementation is in [`python/pro3_24g_gyro.py`](../python/pro3_24g_gyro.py)
(a small reader) and [`python/pro3_bridge.py`](../python/pro3_bridge.py) (the full bridge).
