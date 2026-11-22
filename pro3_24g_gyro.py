"""
pro3_24g_gyro.py -- Activa y lee el giroscopio + acelerometro + botones + sticks + gatillos
del 8BitDo Pro 3 por su dongle 2.4 GHz en Windows (SIN modificar firmware).

Como funciona (descubierto del firmware v1.02 del dongle JieLi AC695x):
  1) Envia [0x81, 0x06, 0x08, 0x00, ...] (WIRELESS_ENTER_TEST_MODE_CMD) por HID vendor 0xFF7A.
  2) El dongle pasa en RAM a modo 2DC8:600A (desactivando el watchdog) y activa el flag
     interno de telemetria completa (200 Hz por Report ID 0x02):
       - bytes 18..21: mascara de botones (uint32 LE)
       - bytes 22..25: sticks LX, LY, RX, RY (uint8)
       - bytes 26..27: gatillos analogicos L2, R2 (uint8)
       - bytes 28..33: acelerometro X, Y, Z (3x int16 LE, 4096 LSB = 1g)
       - bytes 34..39: giroscopio  X, Y, Z (3x int16 LE, +/-2000 deg/s = +/-32767)
       - byte  41:     bateria (%)
  3) Es 100% reversible: ejecuta `python pro3_24g_gyro.py --xinput` o desenchufa el dongle.

Uso:
  python pro3_24g_gyro.py               # activa modo gyro 2.4G y muestra lecturas en vivo
  python pro3_24g_gyro.py --seconds 5   # lee durante N segundos
  python pro3_24g_gyro.py --xinput      # vuelve el dongle a XInput (2DC8:310B)
"""
import argparse
import struct
import time
import hid

ACCEL_SCALE = 4096.0          # LSB / g
GYRO_DPS_SCALE = 2000.0 / 32767.0  # deg/s per LSB


def open_dongle_for_gyro(wait_timeout=6.0):
    # If already in 2DC8:600A, check if it's already streaming Report ID 0x02
    devs_600a = hid.enumerate(0x2DC8, 0x600A)
    if devs_600a:
        h = hid.device()
        h.open_path(devs_600a[0]["path"])
        h.set_nonblocking(True)
        # Check briefly for Report ID 0x02
        t0 = time.time()
        got_02 = False
        while time.time() - t0 < 0.25:
            d = h.read(64)
            if d and d[0] == 0x02 and len(d) >= 42:
                got_02 = True
                break
            time.sleep(0.005)
        if got_02:
            return h
        # Send WIRELESS_ENTER_TEST_MODE_CMD to enable 0x02 telemetry stream
        try:
            h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))
        except Exception:
            pass
        h.close()
        time.sleep(0.3)
    else:
        devs_310b = [d for d in hid.enumerate(0x2DC8, 0x310B) if d["usage_page"] == 0xFF7A]
        if not devs_310b:
            if hid.enumerate(0x2DC8, 0x3109):
                raise RuntimeError("El dongle esta en espera (2DC8:3109). Enciende el mando primero.")
            raise RuntimeError("No se encontro el dongle 8BitDo Pro 3 (ni 2DC8:310B ni 2DC8:600A).")
        h = hid.device()
        h.open_path(devs_310b[0]["path"])
        h.write(bytes([0x81, 0x06, 0x08, 0x00]) + bytes(60))
        h.close()
        print("Comando de telemetria gyro enviado; esperando re-enumeracion 2DC8:600A...")

    deadline = time.time() + wait_timeout
    while time.time() < deadline:
        devs = hid.enumerate(0x2DC8, 0x600A)
        if devs:
            time.sleep(0.2)
            h = hid.device()
            h.open_path(devs[0]["path"])
            h.set_nonblocking(True)
            return h
        time.sleep(0.05)
    raise RuntimeError("2DC8:600A no aparecio a tiempo.")


def switch_back_xinput():
    devs = hid.enumerate(0x2DC8, 0x600A)
    if not devs:
        if hid.enumerate(0x2DC8, 0x310B):
            print("El dongle ya esta en modo XInput (2DC8:310B).")
            return
        print("No se encontro el dongle en 2DC8:600A.")
        return
    h = hid.device()
    h.open_path(devs[0]["path"])
    h.write(bytes([0x81, 0x00, 0x51, 0x02]) + bytes(60))
    h.close()
    print("Enviado comando de retorno a XInput (2DC8:310B). Esperando 2.5s...")
    time.sleep(2.5)
    if hid.enumerate(0x2DC8, 0x310B):
        print("OK: dongle de vuelta en XInput (2DC8:310B).")
    else:
        print("Aviso: si el dongle paso a espera (2DC8:3109), toca un boton o desenchufa/re-enchufa.")


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
        "sticks": (lx, ly, rx, ry),
        "triggers": (l2, r2),
        "accel_g": (ax / ACCEL_SCALE, ay / ACCEL_SCALE, az / ACCEL_SCALE),
        "gyro_dps": (gx * GYRO_DPS_SCALE, gy * GYRO_DPS_SCALE, gz * GYRO_DPS_SCALE),
        "accel_raw": (ax, ay, az),
        "gyro_raw": (gx, gy, gz),
        "battery": bat,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--seconds", type=float, default=10.0, help="Duracion de lectura (0 = infinito)")
    ap.add_argument("--xinput", action="store_true", help="Volver el dongle a XInput (2DC8:310B)")
    args = ap.parse_args()

    if args.xinput:
        switch_back_xinput()
        return

    h = open_dongle_for_gyro()
    print("Conectado a 2DC8:600A (2.4 GHz). Leyendo giroscopio en vivo (Ctrl+C para salir)...")
    t0 = time.time()
    last_print = 0.0
    count = 0
    try:
        while args.seconds == 0 or (time.time() - t0) < args.seconds:
            d = h.read(64)
            if d:
                st = parse_telemetry(d)
                if st:
                    count += 1
                    now = time.time()
                    if now - last_print >= 0.2:
                        ax, ay, az = st["accel_g"]
                        gx, gy, gz = st["gyro_dps"]
                        lx, ly, rx, ry = st["sticks"]
                        l2, r2 = st["triggers"]
                        print(
                            f"  gyro(dps)=({gx:+7.1f},{gy:+7.1f},{gz:+7.1f})  "
                            f"accel(g)=({ax:+5.2f},{ay:+5.2f},{az:+5.2f})  "
                            f"sticks=({lx:3d},{ly:3d},{rx:3d},{ry:3d})  "
                            f"trig=({l2:3d},{r2:3d})  btns=0x{st['btns']:08x}  bat={st['battery']}%"
                        )
                        last_print = now
            else:
                time.sleep(0.001)
    except KeyboardInterrupt:
        pass
    finally:
        h.close()
    dt = max(time.time() - t0, 1e-3)
    print(f"Leidos {count} paquetes de telemetria en {dt:.2f}s ({count/dt:.1f} Hz).")


if __name__ == "__main__":
    main()
