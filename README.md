# ⌨️ Windows Remote Keyboard & Trackpad (Rust)

Ein extrem leichtgewichtiger, portabler Webserver in Rust, mit dem du diesen und andere Windows-Rechner bequem und mit minimaler Latenz aus der Ferne über dein **iPad** oder **iPhone** steuern kannst.

Die Benutzeroberfläche ist als moderne, OLED-schwarze Progressive Web App (PWA) gestaltet und bietet ein vollständiges Multi-Touch-Trackpad mit Tap-to-Drag, eine vollwertige deutsche Tastatur (QWERTZ) inklusive Umlauten, Windows-Sondertasten, Shortcuts und Mediensteuerung.

Das Projekt umfasst zwei ausführbare Varianten:
1. **`remote-keyboard.exe` (Konsolen-App):** Zeigt beim Start einen scanbaren ASCII-QR-Code im Terminal an.
2. **`remote-keyboard-tray.exe` (System-Tray Version):** Läuft dezent im Hintergrund in der Windows-Taskleiste neben der Uhr, unterstützt Windows-Autostart, speichert den Token persistent und zeigt den QR-Code in einem schlanken nativen Fenster an.

---

## ✨ Features

- **🚀 All-in-One Portable Binary:** Keine Installation oder Laufzeit-Abhängigkeiten. Alle Web-Dateien sind direkt in die kompilierte `.exe` eingebettet.
- **⚡ Extrem geringe Latenz (< 5 ms):** Direkte WebSocket-Kommunikation mit optimierter Touch-Ereignis-Verarbeitung über `requestAnimationFrame`.
- **🖤 Echtes OLED-Schwarz (`#000000`):** Perfekter Kontrast und maximale Energieeinsparung auf iPhone- und iPad-OLED-Displays.
- **👆 Multi-Touch Trackpad-Gesten:**
  - **1 Finger:** Cursor bewegen & sanfte Beschleunigung
  - **1-Finger-Tippen:** Linksklick
  - **2-Finger-Tippen:** Rechtsklick
  - **2-Finger-Wischen:** Scrollen nach Windows-Standard (Wischen nach unten = Dokument nach unten)
  - **Doppeltippen & Halten/Ziehen (Tap-to-Drag):** Natives Drag & Drop direkt auf dem Trackpad mit blauem Leuchtrahmen und taktilem Feedback
  - **Maustasten:** Große Daumentasten für Linksklick, Rechtsklick und mittleren Klick.
- **🔤 Vollständige deutsche Tastatur (QWERTZ):**
  - F-Tasten (F1 bis F12)
  - Zahlenreihe & Sonderzeichen (`^`, `1`..`0`, `ß`, `´`, etc.)
  - Deutsche Umlaute (`Ä`, `Ö`, `Ü`) und Sonderzeichen (`#`, `-`, `,`, `.`)
  - Einrastende Modifikatoren: `Strg`, `Alt`, `Shift`, `⊞ Win` und `AltGr`
  - Windows-Shortcuts: `Alt+Tab`, `Win+D` (Desktop), `Win+E` (Explorer), `Win+L` (Sperren)
  - Mediensteuerung: Lautstärke lauter/leiser, Stumm (Mute), Vorheriger Titel (`⏮️`), Play/Pause (`⏯️`), Nächster Titel (`⏭️`)
- **🎤 iOS-Tastatur & Diktierfunktion:** Über das Tastatur-Icon lässt sich jederzeit das native iOS-Tastaturfeld öffnen, um Siri-Spracheingaben oder iOS-Wortvorschläge direkt auf den PC zu übertragen.
- **🔔 Windows System-Tray Integration:**
  - Kein störendes Konsolenfenster
  - Startet optional automatisch beim Windows-Login (`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`)
  - Rechnerneustart-fester Token (iPad verbindet sich nach PC-Neustart automatisch wieder)
  - Nativer Windows-Dialog zum Anzeigen des QR-Codes und Kopieren der URL.

---

## 🚀 Schnellstart

### Variante A: System-Tray Version (Empfohlen für den Alltag)
Starte die Tray-Version per Doppelklick auf `start-tray.bat` oder direkt:
```powershell
.\target\release\remote-keyboard-tray.exe
```
- Die App minimiert sich sofort in die Windows-Taskleiste (neben der Uhr).
- **Linksklick / Doppelklick:** Öffnet das native QR-Code Fenster.
- **Rechtsklick:** Öffnet das Menü (QR-Code anzeigen, URL kopieren, Mit Windows starten, Token generieren, Beenden).

### Variante B: Konsolen-Version (Ideal für schnelle Tests)
Starte die Konsolen-Version über `start.bat`:
```powershell
.\target\release\remote-keyboard.exe
```
Druckt den QR-Code direkt in die Konsole.

---

## 📱 Verbinden mit dem iPad / iPhone
1. Öffne die **Kamera-App** auf deinem iPad oder iPhone und richte sie auf den QR-Code.
2. Tippe auf die angezeigte Benachrichtigung, um die Fernsteuerung in Safari zu öffnen.
3. *(Empfohlen)* Tippe in Safari auf das **Teilen-Symbol** (Viereck mit Pfeil nach oben) und wähle **„Zum Home-Bildschirm“**. Dadurch läuft die Steuerung im randlosen Vollbildmodus ohne Browserleisten.

---

## 🛠️ Aus dem Quellcode bauen

```powershell
# Beide Versionen im Release-Modus kompilieren
cargo build --release
```
Erzeugt:
- `target/release/remote-keyboard.exe` (CLI-Version)
- `target/release/remote-keyboard-tray.exe` (Tray-Version)
