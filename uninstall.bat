@echo off
setlocal
cd /d "%~dp0"
echo =======================================================
echo    Windows Remote Steuerung - Deinstallation
echo =======================================================
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0uninstall.ps1"
echo.
pause
