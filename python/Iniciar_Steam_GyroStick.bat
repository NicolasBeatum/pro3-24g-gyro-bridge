@echo off
title 8BitDo Pro 3 - Steam Gyro-a-Stick (Mantener L2 + DSU 200Hz)
cd /d "%~dp0"
python pro3_bridge.py --gyro stick --trigger lt --sens 1.4
pause
