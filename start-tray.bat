@echo off
if not exist "target\release\remote-keyboard-tray.exe" (
    echo Kompiliere Tray-Version... Bitte kurz warten...
    cargo build --release --bin remote-keyboard-tray
)
start "" "target\release\remote-keyboard-tray.exe"
