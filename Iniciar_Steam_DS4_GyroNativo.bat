@echo off
title 8BitDo Pro 3 - Steam DualShock 4 con Gyro Nativo + DSU Emuladores (200Hz)
cd /d "%~dp0"
python pro3_bridge.py --pad ds4 --gyro off
pause
