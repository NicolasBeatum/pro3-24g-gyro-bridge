# 8BitDo Pro 3 · 2.4 GHz Gyro Bridge (Windows)

<p align="center">
  <img src="assets/8bitdo_pro3.svg" alt="8BitDo Pro 3 2.4 GHz Gyro Bridge" width="520"/>
</p>

<p align="center">
  <a href="README.md">English</a> | <b>Español</b>
</p>

Habilita el **giroscopio + acelerómetro a 200 Hz**, **vibración dual**, **gatillos analógicos** y **botones traseros independientes (PL/PR)** del **8BitDo Pro 3** a través de su **dongle USB 2.4 GHz** en Windows — **sin modificar ni flashear firmware** (100% en RAM y reversible).

## Características

- **Steam Input con Giroscopio Nativo:** emula un **DualShock 4** virtual vía ViGEmBus (`DS4_REPORT_EX`) con giroscopio 3 ejes + acelerómetro 3 ejes reales a **200 Hz** (aparece el menú oficial de Giroscopio, calibración, Flick Stick y perfiles por juego en Steam).
- **Emuladores (DSU / Cemuhook UDP `127.0.0.1:26760`):** servidor integrado a 200 Hz compatible con **Cemu, Dolphin, Ryujinx, Yuzu / Eden / Sudachi, RPCS3 y Citra/Azahar**, en paralelo con el mando virtual.
- **Vibración bidireccional:** reenvía la vibración de ambos motores desde Steam/PC y desde emuladores DSU hacia el dongle 2.4 GHz.
- **Gatillos analógicos completos (`L2 / R2` `0..255`):** conserva el recorrido analógico completo a diferencia del modo Switch.
- **Botones traseros (`PL / PR`):** monitor en vivo y remapeo a DS4 Touchpad Click, Recalibrar Gyro, Pausar Gyro, botones del mando o teclas del teclado (`Espacio`, `Shift`, `F12`, etc.).
- **Modo Gyro directo para PC (opcional):** Gyro→Mouse (`SendInput`) y Gyro→Stick Derecho para juegos fuera de Steam o en modo Xbox 360.
- **Interfaz nativa en Rust (`pro3-bridge.exe`):** vista interactiva en vivo del 8BitDo Pro 3, horizonte artificial de giroscopio a 200 Hz, lectura HID por evento USB (~0.2 ms), minimizado a la bandeja del sistema con aviso al cerrar, interfaz bilingüe **ES / EN** y configuración persistente (`pro3_bridge_config.json`).

## Cómo funciona (100% seguro y reversible)

El firmware oficial del dongle 2.4 GHz (JieLi AC695x) incluye un modo de telemetría en RAM (`2DC8:600A`) activable enviando el comando HID vendor `[0x81, 0x06, 0x08, 0x00]` por `Usage Page 0xFF7A`:
- No modifica flash ni firmware del dongle ni del mando.
- Emite paquetes `Report ID 0x02` a ~200 Hz con botones (32 bits), sticks, gatillos analógicos, acelerómetro (`4096 LSB/g`), giroscopio (`±2000 °/s`) y batería.
- Acepta vibración por `Report ID 0x81` (`CUSTOM_CMD 0x08`).
- Para volver el dongle al modo XInput de fábrica (`2DC8:310B`), simplemente cerrá la app y **desenchufá / volvé a enchufar el dongle**.

Documentación completa a nivel de bytes (modos USB, comando de activación, estructura de telemetría de 64 bytes, máscara de botones, conversión de ejes de sensores, vibración): **[docs/PROTOCOL.md](docs/PROTOCOL.md)** (en inglés).

## Estructura del proyecto

```
.
├── pro3-bridge/          # App nativa en Rust con GUI (pro3-bridge.exe)
│   ├── Cargo.toml
│   └── src/              # protocol.rs · dsu.rs · vigem.rs · bridge.rs · ui.rs · config.rs · input_sim.rs
├── python/               # Implementación de referencia en Python + lanzadores .bat
│   ├── pro3_bridge.py    # Puente completo (servidor DSU, DS4/X360 virtual, gyro→mouse/stick)
│   ├── pro3_24g_gyro.py  # Lector mínimo de telemetría
│   └── *.bat             # Lanzadores de un clic (Steam / emuladores / volver a XInput)
├── docs/
│   └── PROTOCOL.md       # Referencia del protocolo HID del dongle 2.4 GHz
├── assets/
│   ├── 8bitdo_pro3.svg   # Ilustración del mando (incrustada en la GUI)
│   ├── app_icon.svg      # Ícono de la app (ventana + bandeja)
│   └── app_icon.ico      # Ícono multitamaño incrustado en pro3-bridge.exe
├── .github/workflows/    # Compilación de release para Windows (CI)
├── README.md / README_ES.md
└── LICENSE
```

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
1. Abrí `pro3-bridge.exe` (modo por defecto: **DualShock 4 — Gyro Nativo Steam**).
2. En **Steam → Parámetros → Mando**, activá **Steam Input para mandos de PlayStation**.
3. Steam detectará el **DualShock 4** con su opción nativa de **Giroscopio** y **Vibración**.

### Emuladores (DSU / Cemuhook)
Con la app abierta (servidor UDP activo en `127.0.0.1:26760`):
- **Dolphin:** *Mandos → Fuentes de entrada alternativas → Habilitar → Añadir* `127.0.0.1:26760`.
- **Cemu:** *Opciones → Entrada → API: DSUController* o *Motion Source → 127.0.0.1:26760*.
- **Ryujinx / Yuzu / Eden:** *Controles → Movimiento → CemuHookUDP →* `127.0.0.1:26760`.

### Versión Python incluida
La implementación de referencia en Python está en [`python/`](python/) (`pro3_bridge.py`, `pro3_24g_gyro.py` y lanzadores `.bat`):
```powershell
pip install hidapi vgamepad
python python/pro3_bridge.py
```
O hacé doble clic en un lanzador dentro de `python/`:

| Lanzador | Modo |
|---|---|
| `Iniciar_Steam_DS4_GyroNativo.bat` | DualShock 4 virtual con gyro nativo en Steam + DSU |
| `Iniciar_Emuladores_DSU.bat` | Servidor DSU para emuladores + mando virtual |
| `Iniciar_Steam_GyroMouse.bat` | Gyro → mouse (mantener L2) + DSU |
| `Iniciar_Steam_GyroStick.bat` | Gyro → stick derecho (mantener L2) + DSU |
| `Volver_a_XInput.bat` | Intenta volver el dongle a XInput (desenchufar y enchufar siempre funciona) |

## Licencia

[MIT](LICENSE)
