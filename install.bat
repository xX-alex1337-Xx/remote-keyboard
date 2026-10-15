@echo off
setlocal
cd /d "%~dp0"
echo =======================================================
echo     Windows Remote Steuerung - Installation
echo =======================================================
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0install.ps1"
echo.
pause
