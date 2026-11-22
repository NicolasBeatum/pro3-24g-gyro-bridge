# 8BitDo Pro 3 · 2.4 GHz Gyro Bridge (Windows)

<p align="center">
  <img src="assets/8bitdo_pro3.svg" alt="8BitDo Pro 3 2.4 GHz Gyro Bridge" width="520"/>
</p>

<p align="center">
  <b>English</b> | <a href="README_ES.md">Español</a>
</p>

Unlock the **200 Hz 6-axis gyroscope + accelerometer**, **dual rumble**, **analog triggers**, and **independent back paddles (PL/PR)** on your **8BitDo Pro 3** over its stock **2.4 GHz USB dongle** on Windows — **zero firmware flashing** (100% RAM-only and reversible).

## Features

- **Native Steam Input Gyroscope:** Emulates a virtual **DualShock 4** via ViGEmBus (`DS4_REPORT_EX`) with real 3-axis gyro + 3-axis accelerometer at **200 Hz** — Steam's native Gyro menu, calibration, Flick Stick, Gyro-as-Mouse, and per-game profiles all work out of the box.
- **Emulators (DSU / Cemuhook UDP `127.0.0.1:26760`):** Built-in 200 Hz motion + rumble server for **Cemu, Dolphin, Ryujinx, Yuzu / Eden / Sudachi, RPCS3, and Citra/Azahar**, running in parallel with the virtual controller.
- **Bidirectional Dual Rumble:** Forwards left/right motor vibration from Steam/PC games and DSU emulators back to the 2.4 GHz dongle.
- **Full Analog Triggers (`L2 / R2` `0..255`):** Keeps full analog trigger range unlike Switch mode.
- **Back Paddles (`PL / PR`):** Live monitor and remapper to DS4 Touchpad Click, Recalibrate Gyro, Pause Gyro, gamepad buttons, or keyboard keys (`Space`, `Shift`, `F12`, etc.).
- **Optional Direct PC Gyro Modes:** Built-in 200 Hz Gyro→Mouse (`SendInput`) and Gyro→Right Stick for non-Steam games or Xbox 360 mode.
- **Native Rust GUI (`pro3-bridge.exe`):** Live interactive 8BitDo Pro 3 controller visualizer, 200 Hz artificial horizon gauge, event-driven USB HID reads (~0.2 ms overhead), Windows System Tray support with first-time close confirmation, bilingual **EN / ES** interface, and persistent config (`pro3_bridge_config.json`).

## How It Works (Safe & 100% Reversible)

The stock 2.4 GHz dongle firmware (JieLi AC695x) includes an in-RAM telemetry mode (`2DC8:600A`) activated via HID vendor command `[0x81, 0x06, 0x08, 0x00]` on `Usage Page 0xFF7A`:
- Does **not** modify flash or firmware on the dongle or controller.
- Streams `Report ID 0x02` at ~200 Hz containing buttons (32-bit), analog sticks, analog triggers, accelerometer (`4096 LSB/g`), gyroscope (`±2000 °/s`), and battery %.
- Accepts dual-motor rumble via `Report ID 0x81` (`CUSTOM_CMD 0x08`).
- To restore stock XInput mode (`2DC8:310B`), simply exit the app and **unplug & replug the USB dongle**.

## Requirements

- **Windows 10 / 11 (64-bit)**
- Signed **[ViGEmBus](https://github.com/nefarius/ViGEmBus/releases)** driver (for virtual DualShock 4 / Xbox 360 in Steam; optional if you only use DSU in Dolphin/Cemu).

## Build from Source (Rust)

```powershell
cd pro3-bridge
cargo build --release
```

Output binary: `pro3-bridge/target/release/pro3-bridge.exe` (single standalone `.exe`, zero external DLLs).

## Quick Setup

### Steam (Native Gyro)
1. Launch `pro3-bridge.exe` (default mode: **DualShock 4 — Native Steam Gyro**).
2. In **Steam → Settings → Controller**, enable **Steam Input for PlayStation controllers**.
3. Steam will detect a **DualShock 4** with native **Gyroscope** calibration, aiming, and **Rumble**.

### Emulators (DSU / Cemuhook)
With `pro3-bridge.exe` running (UDP server active on `127.0.0.1:26760`):
- **Dolphin:** *Controllers → Alternate Input Sources → Enable → Add* `127.0.0.1:26760`.
- **Cemu:** *Options → Input Settings → API: DSUController* or *Motion Source → 127.0.0.1:26760*.
- **Ryujinx / Yuzu / Eden:** *Controls → Motion → Use CemuHookUDP →* `127.0.0.1:26760`.

### Python Reference Implementation
A standalone Python implementation (`pro3_bridge.py` + `pro3_24g_gyro.py` and `.bat` launchers) is also included:
```powershell
pip install hidapi vgamepad
python pro3_bridge.py
```

## License

[MIT](LICENSE)
