# 8BitDo Pro 3 · 2.4 GHz Gyro Bridge (Windows)

<p align="center">
  <img src="assets/8bitdo_pro3.svg" alt="8BitDo Pro 3 2.4 GHz Gyro Bridge" width="520"/>
</p>

Habilita el **giroscopio + acelerómetro a 200 Hz**, **vibración dual**, **gatillos analógicos** y **botones traseros (PL/PR)** del **8BitDo Pro 3** a través de su **dongle USB 2.4 GHz** en Windows — **sin modificar ni flashear firmware** (100% en RAM y reversible).

## Características

- **Steam Input con Giroscopio Nativo:** emula un **DualShock 4** virtual vía ViGEmBus (`DS4_REPORT_EX`) con giroscopio 3 ejes + acelerómetro 3 ejes reales a **200 Hz** (aparece el menú oficial de Giroscopio, calibración, Flick Stick y perfiles por juego en Steam).
- **Emuladores (DSU / Cemuhook UDP `127.0.0.1:26760`):** servidor integrado a 200 Hz compatible con **Cemu, Dolphin, Ryujinx, Yuzu / Eden / Sudachi, RPCS3 y Citra/Azahar**, en paralelo con el mando virtual.
- **Vibración bidireccional:** reenvía la vibración de ambos motores desde Steam/PC y desde emuladores DSU hacia el dongle 2.4 GHz.
- **Gatillos analógicos completos (`L2 / R2` `0..255`)** y todos los botones (sin el límite digital del modo Switch).
- **Botones extra (`P1 / P2 / P3 / P4` — `L4 / R4 / PL / PR`):** monitor en vivo y remapeo a Touchpad Click, Recalibrar Gyro, Pausar Gyro, botones del mando o teclas del teclado (`Espacio`, `Shift`, `F12`, etc.).
- **Modo Gyro directo para PC (opcional):** Gyro→Mouse (`SendInput`) y Gyro→Stick Derecho para juegos fuera de Steam o en modo Xbox 360.
- **Interfaz nativa en Rust (`pro3-bridge.exe`):** lectura HID bloqueante por evento USB (~0.2 ms de overhead), calibración automática con detección de movimiento, auto-recuperación si el mando entra en reposo y configuración persistente (`pro3_bridge_config.json`).

## Cómo funciona (100% seguro y reversible)

El firmware oficial del dongle 2.4 GHz (JieLi AC695x) incluye un modo de telemetría en RAM (`2DC8:600A`) activable enviando el comando HID vendor `[0x81, 0x06, 0x08, 0x00]` por `Usage Page 0xFF7A`:
- No modifica flash ni firmware del dongle ni del mando.
- Emite paquetes `Report ID 0x02` a ~200 Hz con botones (32 bits), sticks, gatillos analógicos, acelerómetro (`4096 LSB/g`), giroscopio (`±2000 °/s`) y batería.
- Acepta vibración por `Report ID 0x81` (`CUSTOM_CMD 0x08`).
- Para volver el dongle al modo XInput de fábrica (`2DC8:310B`), simplemente cerrá la app y **desenchufá / volvé a enchufar el dongle**.

## Requisitos

- **Windows 10 / 11 (64-bit)**
- Driver firmado **[ViGEmBus](https://github.com/nefarius/ViGEmBus/releases)** (para el mando virtual DualShock 4 / Xbox 360 en Steam; opcional si solo usás DSU en Dolphin/Cemu).

## Compilar el `.exe` en Rust

```powershell
cd pro3-bridge
cargo build --release
```

El ejecutable queda en `pro3-bridge/target/release/pro3-bridge.exe` (sin dependencias externas ni DLLs).

## Uso rápido

### Steam (Giroscopio nativo)
1. Abrí `pro3-bridge.exe` (o `Iniciar_Steam_DS4_GyroNativo.bat` si usás la versión Python).
2. En **Steam → Parámetros → Mando**, activá **Steam Input para mandos de PlayStation**.
3. Steam detectará el **DualShock 4** con su opción nativa de **Giroscopio** y **Vibración**.

### Emuladores (DSU / Cemuhook)
Con la app abierta (servidor UDP activo en `127.0.0.1:26760`):
- **Dolphin:** *Mandos → Fuentes de entrada alternativas → Habilitar → Añadir* `127.0.0.1:26760`.
- **Cemu:** *Opciones → Entrada → API: DSUController* o *Motion Source → 127.0.0.1:26760*.
- **Ryujinx / Yuzu / Eden:** *Controles → Movimiento → CemuHookUDP →* `127.0.0.1:26760`.

### Versión Python incluida
También se incluye la implementación de referencia en Python (`pro3_bridge.py` + `pro3_24g_gyro.py` y lanzadores `.bat`):
```powershell
pip install hidapi vgamepad
python pro3_bridge.py
```

## Licencia

[MIT](LICENSE)
