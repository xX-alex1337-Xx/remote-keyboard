# =======================================================
# Remote Keyboard - Deinstaller Script
# Entfernt die Windows System-Tray App restlos vom System
# =======================================================

$ErrorActionPreference = "Continue"

$InstallDir = "$env:LOCALAPPDATA\Programs\RemoteKeyboard"
$StartMenuDir = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs"
$ShortcutPath = Join-Path $StartMenuDir "Windows Remote Steuerung.lnk"

Write-Host "`n[1/4] Beende laufende Remote Keyboard Prozesse..." -ForegroundColor Yellow
Get-Process -Name 'remote-keyboard-tray' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 600

Write-Host "[2/4] Entferne Autostart-Eintrag aus der Windows-Registry..." -ForegroundColor Yellow
Remove-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "RemoteKeyboardTray" -ErrorAction SilentlyContinue

Write-Host "[3/4] Entferne Startmenü-Verknüpfung..." -ForegroundColor Yellow
if (Test-Path $ShortcutPath) {
    Remove-Item -Path $ShortcutPath -Force -ErrorAction SilentlyContinue
}

Write-Host "[4/4] Entferne Programmdateien..." -ForegroundColor Yellow
if (Test-Path $InstallDir) {
    # Falls das Skript selbst aus InstallDir ausgeführt wird, verzögert löschen
    Remove-Item -Path "$InstallDir\remote-keyboard-tray.exe" -Force -ErrorAction SilentlyContinue
    Remove-Item -Path $InstallDir -Recurse -Force -ErrorAction SilentlyContinue
}

Write-Host "`n=======================================================" -ForegroundColor Green
Write-Host "  Deinstallation erfolgreich abgeschlossen!" -ForegroundColor Green
Write-Host "=======================================================" -ForegroundColor Green
Write-Host "Remote Keyboard wurde sauber von deinem System entfernt.`n"
