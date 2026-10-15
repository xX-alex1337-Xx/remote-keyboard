@echo off
title Windows Remote Steuerung (Rust Server)
cls
if not exist "target\release\remote-keyboard.exe" (
    echo Kompiliere Server... Bitte kurz warten...
    cargo build --release
)
"target\release\remote-keyboard.exe"
pause
