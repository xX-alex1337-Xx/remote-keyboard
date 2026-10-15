# ⌨️ Windows Remote Keyboard & Trackpad (Rust)

Ein extrem leichtgewichtiger, portabler Webserver in Rust, mit dem du diesen und andere Windows-Rechner bequem und mit minimaler Latenz aus der Ferne über dein **iPad** oder **iPhone** steuern kannst.

Die Benutzeroberfläche ist als moderne, dunkle Progressive Web App (PWA) gestaltet und bietet ein vollständiges Multi-Touch-Trackpad, eine vollwertige deutsche Tastatur (QWERTZ) inklusive Umlauten, Windows-Sondertasten, Shortcuts und Mediensteuerung.

---

## ✨ Features

- **🚀 All-in-One Portable Binary:** Keine Installation nötig. Das gesamte Web-Frontend ist direkt in die kompilierte `remote-keyboard.exe` eingebettet.
- **⚡ Extrem geringe Latenz (< 5 ms):** Direkte WebSocket-Kommunikation mit optimierter Touch-Ereignis-Verarbeitung über `requestAnimationFrame`.
- **🔒 QR-Code & Token-Pairing:** Beim Start generiert der Server einen ASCII-QR-Code im Terminal. Ein kurzer Kamerascan mit dem iPad/iPhone verbindet das Gerät sofort ohne lästiges Tippen.
- **📱 PWA & Home-Screen Vollbild:** Kann in Safari mit *„Zum Home-Bildschirm“* gespeichert werden. Öffnet sich als eigenständige App im Vollbildmodus ohne störende Safari-Adressleiste.
- **👆 Multi-Touch Trackpad-Gesten:**
  - **1 Finger:** Cursor bewegen & Beschleunigung
  - **1-Finger-Tippen:** Linksklick
  - **2-Finger-Tippen:** Rechtsklick
  - **2-Finger-Wischen:** Sanftes Scrollen (vertikal und horizontal mit natürlicher Scrollrichtung)
  - **Drag-Lock:** Taste zum Einrasten der linken Maustaste (ideal für Fensterverschieben oder Markieren)
  - **Maustasten:** Große Daumentasten für Linksklick, Rechtsklick und mittleren Klick.
- **🔤 Vollständige deutsche Tastatur (QWERTZ):**
  - F-Tasten (F1 bis F12)
  - Zahlenreihe & Sonderzeichen (`^`, `1`..`0`, `ß`, `´`, etc.)
  - Deutsche Umlaute (`Ä`, `Ö`, `Ü`) und Sonderzeichen (`#`, `-`, `,`, `.`)
  - Einrastende Modifikatoren: `Strg`, `Alt`, `Shift`, `⊞ Win` und `AltGr`
  - Windows-Shortcuts: `Alt+Tab`, `Win+D` (Desktop), `Win+E` (Explorer), `Win+L` (Sperren)
  - Mediensteuerung: Lautstärke lauter/leiser, Stumm (Mute), Play/Pause
- **🎤 iOS-Tastatur & Diktierfunktion:** Über das Tastatur-Icon lässt sich jederzeit das native iOS-Tastaturfeld öffnen, um die iOS-Autokorrektur oder die Siri-Spracheingabe (Diktieren) direkt auf den Windows-PC zu übertragen.
- **📳 Taktiles Feedback:** Haptische Vibrationen auf unterstützten Mobilgeräten bei Klicks und Tastendrücken.

---

## 🚀 Schnellstart

### 1. Ausführen (Windows Host)
Starte die Executable in der Konsole oder per Doppelklick:
```powershell
.\target\release\remote-keyboard.exe
```
Der Server ermittelt automatisch deine lokale Netzwerk-IP (z. B. `192.168.178.42`) und zeigt einen QR-Code sowie die Verbindungs-URL an:

```text
=======================================================
     WINDOWS REMOTE-STEUERUNG (RUST SERVER)
=======================================================

Scanne diesen QR-Code mit der Kamera deines iPads/iPhones,
um die Fernsteuerung direkt im Browser zu öffnen:

[ █▀▀▀▀▀█ ... ASCII QR-CODE ... █▀▀▀▀▀█ ]

-------------------------------------------------------
Manuelle Verbindungs-URL:
  http://192.168.178.42:8080/?token=abc123xyz

Sicherheitstoken (Token):
  abc123xyz
-------------------------------------------------------
```

### 2. Verbinden (iPad / iPhone)
1. Öffne die **Kamera-App** auf deinem iPad oder iPhone und halte sie auf den QR-Code in der Konsole.
2. Tippe auf den erscheinenden Safari-Link.
3. *(Empfohlen)* Tippe in Safari auf das **Teilen-Symbol** (Viereck mit Pfeil nach oben) und wähle **„Zum Home-Bildschirm“**. Dadurch läuft die Steuerung im echten Vollbildmodus ohne Browserbalken.

---

## 🛠️ Aus dem Quellcode bauen

Voraussetzungen:
- Rust Toolchain (`cargo`, `rustc`)

```powershell
# Repository klonen
git clone https://github.com/xX-alex1337-Xx/remote-keyboard.git
cd remote-keyboard

# Release-Version kompilieren
cargo build --release
```
Die fertige, selbstständige Binärdatei liegt anschließend unter:
`target/release/remote-keyboard.exe`

---

## ⚙️ Konfiguration

Standardmäßig lauscht der Server auf Port `8080`. Der Port kann über die Umgebungsvariable `PORT` angepasst werden:
```powershell
$env:PORT = "9000"
.\target\release\remote-keyboard.exe
```

---

## 🔒 Sicherheit
- Der Server bindet standardmäßig an das lokale Netzwerk (`0.0.0.0:<port>`).
- Jede Verbindung erfordert einen kryptografisch zufälligen 16-stelligen Token, der bei jedem Serverstart neu generiert wird.
- Verbindungsversuche ohne gültigen Token werden sofort mit `401 Unauthorized` abgewiesen.
