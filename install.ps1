# =======================================================
# Remote Keyboard - Installer Script
# Installiert die Windows System-Tray App ohne Admin-Rechte
# =======================================================

$ErrorActionPreference = "Stop"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
if (-not $ScriptDir) { $ScriptDir = Get-Location }

$InstallDir = "$env:LOCALAPPDATA\Programs\RemoteKeyboard"
$StartMenuDir = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs"
$ShortcutPath = Join-Path $StartMenuDir "Windows Remote Steuerung.lnk"
$TargetExe = Join-Path $InstallDir "remote-keyboard-tray.exe"

Write-Host "`n[1/5] Suche Binary..." -ForegroundColor Cyan
$SourceExe = Join-Path $ScriptDir "target\release\remote-keyboard-tray.exe"
if (-not (Test-Path $SourceExe)) {
    $SourceExe = Join-Path $ScriptDir "remote-keyboard-tray.exe"
}

if (-not (Test-Path $SourceExe)) {
    Write-Host "Keine vorkompilierte .exe gefunden. Baue Release-Version mit Cargo..." -ForegroundColor Yellow
    & cargo build --release --bin remote-keyboard-tray
    $SourceExe = Join-Path $ScriptDir "target\release\remote-keyboard-tray.exe"
    if (-not (Test-Path $SourceExe)) {
        Write-Error "Build fehlgeschlagen oder 'remote-keyboard-tray.exe' nicht auffindbar."
        exit 1
    }
}

Write-Host "[2/5] Beende eventuell bereits laufende Instanzen..." -ForegroundColor Cyan
Get-Process -Name 'remote-keyboard-tray' -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 600

Write-Host "[3/5] Installiere Programmdateien nach $InstallDir..." -ForegroundColor Cyan
if (-not (Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}
Copy-Item -Path $SourceExe -Destination $TargetExe -Force

# Optional Deinstallations-Skripte im Zielordner ablegen
if (Test-Path (Join-Path $ScriptDir "uninstall.bat")) {
    Copy-Item (Join-Path $ScriptDir "uninstall.bat") (Join-Path $InstallDir "uninstall.bat") -Force -ErrorAction SilentlyContinue
}
if (Test-Path (Join-Path $ScriptDir "uninstall.ps1")) {
    Copy-Item (Join-Path $ScriptDir "uninstall.ps1") (Join-Path $InstallDir "uninstall.ps1") -Force -ErrorAction SilentlyContinue
}

Write-Host "[4/5] Erstelle Startmenü-Verknüpfung und Autostart..." -ForegroundColor Cyan
# Startmenü
$WshShell = New-Object -ComObject WScript.Shell
$Shortcut = $WshShell.CreateShortcut($ShortcutPath)
$Shortcut.TargetPath = $TargetExe
$Shortcut.WorkingDirectory = $InstallDir
$Shortcut.Description = "Windows Fernsteuerung via iPad/iPhone Tastatur & Trackpad"
$Shortcut.Save()

# Autostart Registry (HKCU)
Set-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name "RemoteKeyboardTray" -Value "`"$TargetExe`"" -Force

Write-Host "[5/5] Starte Remote Keyboard..." -ForegroundColor Cyan
try {
    Invoke-CimMethod -ClassName Win32_Process -MethodName Create -Arguments @{ CommandLine = $TargetExe } | Out-Null
} catch {
    Start-Process -FilePath $TargetExe -WorkingDirectory $InstallDir
}

Write-Host "`n=======================================================" -ForegroundColor Green
Write-Host "  Installation erfolgreich abgeschlossen!" -ForegroundColor Green
Write-Host "=======================================================" -ForegroundColor Green
Write-Host "Die App läuft jetzt im System-Tray (unten rechts neben der Uhr)."
Write-Host "Klicke mit der rechten Maustaste auf das Icon, um den QR-Code"
Write-Host "für dein iPad/iPhone anzuzeigen.`n"
