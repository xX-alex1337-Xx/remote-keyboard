/**
 * Windows Remote Control Client
 * Touchpad gestures, QWERTZ keyboard, shortcuts, media controls, and WebSocket communication.
 */

(function () {
  'use strict';

  // --- State & Configuration ---
  const state = {
    ws: null,
    token: null,
    connected: false,
    pingInterval: null,
    lastPingTime: 0,
    latency: null,
    
    // Einstellungen
    sensitivity: 1.3,
    scrollSpeed: 1.0,
    naturalScroll: false,
    haptics: true,
    
    // Modifier Status (Strg, Alt, Shift, Win)
    modifiers: {
      Control: false,
      Alt: false,
      Shift: false,
      Meta: false,
    },
    capsLock: false,

    // Touchpad Tracking & Tap-to-Drag
    touches: {},
    touchStartTime: 0,
    touchStartCount: 0,
    initialTouchPositions: {},
    totalMovement: 0,
    pendingMouseMove: { dx: 0, dy: 0 },
    rafScheduled: false,

    // Tap-to-Drag State
    lastTapTime: 0,
    lastTapPos: { x: 0, y: 0 },
    singleTapTimer: null,
    potentialDrag: false,
    isDragging: false,
  };

  // Shift & AltGr mappings for German QWERTZ layout
  const germanShiftMap = {
    '^': '°', '1': '!', '2': '"', '3': '§', '4': '$', '5': '%', '6': '&', '7': '/', '8': '(', '9': ')', '0': '=', 'ß': '?', '´': '`',
    'q': 'Q', 'w': 'W', 'e': 'E', 'r': 'R', 't': 'T', 'z': 'Z', 'u': 'U', 'i': 'I', 'o': 'O', 'p': 'P', 'ü': 'Ü', '+': '*',
    'a': 'A', 's': 'S', 'd': 'D', 'f': 'F', 'g': 'G', 'h': 'H', 'j': 'J', 'k': 'K', 'l': 'L', 'ö': 'Ö', 'ä': 'Ä', '#': '\'',
    '<': '>', 'y': 'Y', 'x': 'X', 'c': 'C', 'v': 'V', 'b': 'B', 'n': 'N', 'm': 'M', ',': ';', '.': ':', '-': '_',
  };

  // --- DOM Elements ---
  const el = {
    statusDot: document.getElementById('status-indicator'),
    statusText: document.getElementById('status-text'),
    latencyBadge: document.getElementById('latency-badge'),
    trackpad: document.getElementById('trackpad'),
    touchMarkers: document.getElementById('touch-markers'),
    trackpadHint: document.getElementById('trackpad-hint'),
    
    // Bottom Buttons
    btnMouseLeft: document.getElementById('btn-mouse-left'),
    btnMouseRight: document.getElementById('btn-mouse-right'),
    btnMouseMiddle: document.getElementById('btn-mouse-middle'),
    
    // Header & Tools
    btnNativeKbd: document.getElementById('btn-native-kbd'),
    btnToggleKbd: document.getElementById('btn-toggle-kbd'),
    btnSettings: document.getElementById('btn-settings'),
    btnFullscreen: document.getElementById('btn-fullscreen'),
    hiddenTextInput: document.getElementById('hidden-text-input'),
    
    // Keyboard Drawer
    keyboardDrawer: document.getElementById('keyboard-drawer'),
    btnCloseKbd: document.getElementById('btn-close-kbd'),
    capsLockBtn: document.getElementById('kbd-caps'),
    
    // Modals
    settingsModal: document.getElementById('settings-modal'),
    btnCloseSettings: document.getElementById('btn-close-settings'),
    sliderSensitivity: document.getElementById('slider-sensitivity'),
    labelSensitivity: document.getElementById('label-sensitivity'),
    sliderScrollSpeed: document.getElementById('slider-scroll-speed'),
    labelScrollSpeed: document.getElementById('label-scroll-speed'),
    checkNaturalScroll: document.getElementById('check-natural-scroll'),
    checkHaptics: document.getElementById('check-haptics'),
    inputManualToken: document.getElementById('input-manual-token'),
    btnSaveToken: document.getElementById('btn-save-token'),
    pairingModal: document.getElementById('pairing-modal'),
    inputPairingToken: document.getElementById('input-pairing-token'),
    btnConnectPairing: document.getElementById('btn-connect-pairing'),
  };

  // --- Haptic Feedback ---
  function vibrate(ms = 15) {
    if (state.haptics && navigator.vibrate) {
      try {
        navigator.vibrate(ms);
      } catch (e) {}
    }
  }

  // --- Token & Connection Management ---
  function initToken() {
    const urlParams = new URLSearchParams(window.location.search);
    const urlToken = urlParams.get('token');
    if (urlToken) {
      state.token = urlToken.trim();
      localStorage.setItem('remote_pc_token', state.token);
      // URL bereinigen ohne Neuladen
      window.history.replaceState({}, document.title, window.location.pathname);
    } else {
      state.token = localStorage.getItem('remote_pc_token');
    }
    if (el.inputManualToken) el.inputManualToken.value = state.token || '';
  }

  function connectWebSocket() {
    if (!state.token) {
      showPairingModal(true);
      return;
    }

    const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    const wsUrl = `${protocol}//${window.location.host}/ws?token=${encodeURIComponent(state.token)}`;

    updateStatus('connecting', 'Verbinde...');

    if (state.ws) {
      try { state.ws.close(); } catch (e) {}
    }

    state.ws = new WebSocket(wsUrl);

    state.ws.onopen = () => {
      updateStatus('connected', 'Verbunden');
      showPairingModal(false);
      startPing();
    };

    state.ws.onmessage = (event) => {
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === 'Pong') {
          const rtt = Math.round(performance.now() - state.lastPingTime);
          state.latency = rtt;
          if (el.latencyBadge) {
            el.latencyBadge.textContent = `${rtt} ms`;
            el.latencyBadge.classList.remove('hidden');
          }
        } else if (msg.type === 'AuthError') {
          updateStatus('disconnected', 'Ungültiger Token');
          showPairingModal(true);
        }
      } catch (err) {
        console.error('Fehler beim Parsen der Server-Nachricht:', err);
      }
    };

    state.ws.onclose = () => {
      updateStatus('disconnected', 'Getrennt');
      stopPing();
      setTimeout(() => {
        if (!state.connected && state.token) {
          connectWebSocket();
        }
      }, 2000);
    };

    state.ws.onerror = (err) => {
      console.warn('WebSocket Fehler:', err);
    };
  }

  function updateStatus(status, text) {
    state.connected = status === 'connected';
    if (el.statusDot) {
      el.statusDot.className = `status-dot ${status}`;
    }
    if (el.statusText) {
      el.statusText.textContent = text;
    }
    if (!state.connected && el.latencyBadge) {
      el.latencyBadge.classList.add('hidden');
    }
  }

  function send(msg) {
    if (state.ws && state.ws.readyState === WebSocket.OPEN) {
      state.ws.send(JSON.stringify(msg));
    }
  }

  function startPing() {
    stopPing();
    state.pingInterval = setInterval(() => {
      if (state.connected) {
        state.lastPingTime = performance.now();
        send({ type: 'Ping' });
      }
    }, 3000);
  }

  function stopPing() {
    if (state.pingInterval) {
      clearInterval(state.pingInterval);
      state.pingInterval = null;
    }
  }

  function showPairingModal(show) {
    if (el.pairingModal) {
      el.pairingModal.classList.toggle('hidden', !show);
    }
  }

  // --- Touchpad Engine & Gesten ---
  function initTrackpad() {
    const pad = el.trackpad;

    pad.addEventListener('touchstart', onTouchStart, { passive: false });
    pad.addEventListener('touchmove', onTouchMove, { passive: false });
    pad.addEventListener('touchend', onTouchEnd, { passive: false });
    pad.addEventListener('touchcancel', onTouchCancel, { passive: false });

    // Fallback für Desktop-Browser Testing
    let isMouseDown = false;
    let lastMousePos = { x: 0, y: 0 };
    pad.addEventListener('mousedown', (e) => {
      isMouseDown = true;
      lastMousePos = { x: e.clientX, y: e.clientY };
    });
    window.addEventListener('mousemove', (e) => {
      if (!isMouseDown) return;
      const dx = (e.clientX - lastMousePos.x) * state.sensitivity;
      const dy = (e.clientY - lastMousePos.y) * state.sensitivity;
      lastMousePos = { x: e.clientX, y: e.clientY };
      queueMouseMove(dx, dy);
    });
    window.addEventListener('mouseup', () => {
      isMouseDown = false;
    });
  }

  function onTouchStart(e) {
    e.preventDefault();
    const touchList = e.touches;
    const now = performance.now();
    state.touchStartTime = now;
    state.touchStartCount = touchList.length;
    state.totalMovement = 0;

    state.touches = {};
    state.initialTouchPositions = {};

    for (let i = 0; i < touchList.length; i++) {
      const t = touchList[i];
      state.touches[t.identifier] = { x: t.clientX, y: t.clientY };
      state.initialTouchPositions[t.identifier] = { x: t.clientX, y: t.clientY };
    }

    // Tap-to-Drag Erkennung bei 1 Finger (Doppeltippen & Halten)
    if (touchList.length === 1) {
      const t = touchList[0];
      const timeSinceLastTap = now - state.lastTapTime;
      const distFromLastTap = Math.hypot(t.clientX - state.lastTapPos.x, t.clientY - state.lastTapPos.y);

      // Zweiter Tap innerhalb von 320ms und im Umkreis von 40px
      if (timeSinceLastTap < 320 && distFromLastTap < 40) {
        if (state.singleTapTimer) {
          clearTimeout(state.singleTapTimer);
          state.singleTapTimer = null;
        }
        state.potentialDrag = true;
      } else {
        state.potentialDrag = false;
      }
    } else {
      state.potentialDrag = false;
      if (state.singleTapTimer) {
        clearTimeout(state.singleTapTimer);
        state.singleTapTimer = null;
      }
    }

    if (el.trackpadHint) {
      el.trackpadHint.style.opacity = '0.08';
    }

    updateTouchMarkers(touchList);
  }

  function onTouchMove(e) {
    e.preventDefault();
    const touchList = e.touches;
    updateTouchMarkers(touchList);

    // 1-Finger Mausbewegung oder Drag
    if (touchList.length === 1 && state.touchStartCount === 1) {
      const t = touchList[0];
      const initial = state.initialTouchPositions[t.identifier];
      const prev = state.touches[t.identifier];

      // Drag-Aktivierung prüfen
      if (state.potentialDrag && !state.isDragging && initial) {
        const movedSinceStart = Math.hypot(t.clientX - initial.x, t.clientY - initial.y);
        if (movedSinceStart > 5) {
          state.isDragging = true;
          send({ type: 'MouseDown', button: 'Left' });
          vibrate(25);
          if (el.trackpad) el.trackpad.classList.add('dragging');
        }
      }

      if (prev) {
        let dx = (t.clientX - prev.x) * state.sensitivity;
        let dy = (t.clientY - prev.y) * state.sensitivity;

        // Leichte Beschleunigungskurve bei schnellen Wischern
        const speed = Math.hypot(dx, dy);
        if (speed > 8) {
          const factor = 1.0 + Math.min(speed / 30, 1.2);
          dx *= factor;
          dy *= factor;
        }

        state.totalMovement += Math.hypot(t.clientX - prev.x, t.clientY - prev.y);
        queueMouseMove(dx, dy);
      }
      state.touches[t.identifier] = { x: t.clientX, y: t.clientY };
    }
    // 2-Finger Scrollen (vertikal & horizontal)
    else if (touchList.length === 2) {
      const t1 = touchList[0];
      const t2 = touchList[1];
      const prev1 = state.touches[t1.identifier];
      const prev2 = state.touches[t2.identifier];

      if (prev1 && prev2) {
        const dy1 = t1.clientY - prev1.y;
        const dy2 = t2.clientY - prev2.y;
        const dx1 = t1.clientX - prev1.x;
        const dx2 = t2.clientX - prev2.x;

        // Durchschnittliche Bewegung beider Finger
        let avgDy = (dy1 + dy2) / 2;
        let avgDx = (dx1 + dx2) / 2;

        state.totalMovement += Math.hypot(avgDx, avgDy);

        // Scrollrichtung:
        // Windows Win32 API:
        // delta > 0 = Wheel forward = Scroll UP
        // delta < 0 = Wheel backward = Scroll DOWN
        // Standard Windows: Finger nach UNTEN wischen (avgDy > 0) scrollt nach UNTEN (delta < 0).
        let scrollDy = -avgDy;
        let scrollDx = -avgDx;

        if (state.naturalScroll) {
          scrollDy = avgDy;
          scrollDx = avgDx;
        }

        const scrollMultiplier = 3.5 * state.scrollSpeed;
        send({
          type: 'MouseWheel',
          dx: Math.round(scrollDx * scrollMultiplier),
          dy: Math.round(scrollDy * scrollMultiplier),
        });
      }

      state.touches[t1.identifier] = { x: t1.clientX, y: t1.clientY };
      state.touches[t2.identifier] = { x: t2.clientX, y: t2.clientY };
    }
  }

  function onTouchEnd(e) {
    e.preventDefault();
    const now = performance.now();
    const duration = now - state.touchStartTime;
    const isQuickTap = duration < 250 && state.totalMovement < 12;

    // 1. Wenn aktiver Drag-Vorgang lief: Beenden
    if (state.isDragging) {
      send({ type: 'MouseUp', button: 'Left' });
      state.isDragging = false;
      state.potentialDrag = false;
      state.lastTapTime = 0;
      vibrate(15);
      if (el.trackpad) el.trackpad.classList.remove('dragging');
    }
    // 2. Wenn zweiter Tap ohne Drag-Bewegung losgelassen wurde = Doppelklick
    else if (state.potentialDrag) {
      state.potentialDrag = false;
      state.lastTapTime = 0;
      vibrate(20);
      send({ type: 'MouseClick', button: 'Left' });
      send({ type: 'MouseClick', button: 'Left' });
    }
    // 3. Wenn schneller 1-Finger Tap = Klick (mit kurzem Warten auf möglichen 2. Tap)
    else if (isQuickTap && state.touchStartCount === 1) {
      const touchId = Object.keys(state.initialTouchPositions)[0];
      const pos = touchId ? state.initialTouchPositions[touchId] : { x: 0, y: 0 };
      state.lastTapTime = now;
      state.lastTapPos = { x: pos.x, y: pos.y };

      state.singleTapTimer = setTimeout(() => {
        state.singleTapTimer = null;
        vibrate(15);
        send({ type: 'MouseClick', button: 'Left' });
      }, 260);
    }
    // 4. Wenn schneller 2-Finger Tap = Rechtsklick
    else if (isQuickTap && state.touchStartCount === 2) {
      vibrate(25);
      send({ type: 'MouseClick', button: 'Right' });
    }

    if (e.touches.length === 0) {
      state.touches = {};
      if (el.trackpadHint) {
        el.trackpadHint.style.opacity = '0.35';
      }
    }
    updateTouchMarkers(e.touches);
  }

  function onTouchCancel(e) {
    onTouchEnd(e);
  }

  function queueMouseMove(dx, dy) {
    state.pendingMouseMove.dx += dx;
    state.pendingMouseMove.dy += dy;

    if (!state.rafScheduled) {
      state.rafScheduled = true;
      requestAnimationFrame(() => {
        const roundedX = Math.round(state.pendingMouseMove.dx);
        const roundedY = Math.round(state.pendingMouseMove.dy);

        if (roundedX !== 0 || roundedY !== 0) {
          send({
            type: 'MouseMove',
            dx: roundedX,
            dy: roundedY,
          });
        }

        state.pendingMouseMove.dx = 0;
        state.pendingMouseMove.dy = 0;
        state.rafScheduled = false;
      });
    }
  }

  function updateTouchMarkers(touchList) {
    if (!el.touchMarkers) return;
    el.touchMarkers.innerHTML = '';
    const rect = el.trackpad.getBoundingClientRect();

    for (let i = 0; i < touchList.length; i++) {
      const t = touchList[i];
      const dot = document.createElement('div');
      dot.className = 'touch-dot';
      dot.style.left = `${t.clientX - rect.left}px`;
      dot.style.top = `${t.clientY - rect.top}px`;
      el.touchMarkers.appendChild(dot);
    }
  }

  // --- Maustasten unten ---
  function initMouseButtons() {
    // Linksklick gedrückt halten / loslassen
    addPressEvents(el.btnMouseLeft, () => {
      vibrate(15);
      send({ type: 'MouseDown', button: 'Left' });
    }, () => {
      send({ type: 'MouseUp', button: 'Left' });
    });

    // Rechtsklick
    addPressEvents(el.btnMouseRight, () => {
      vibrate(20);
      send({ type: 'MouseDown', button: 'Right' });
    }, () => {
      send({ type: 'MouseUp', button: 'Right' });
    });

    // Mittlerer Klick
    addPressEvents(el.btnMouseMiddle, () => {
      vibrate(15);
      send({ type: 'MouseDown', button: 'Middle' });
    }, () => {
      send({ type: 'MouseUp', button: 'Middle' });
    });
  }

  function addPressEvents(element, onDown, onUp) {
    if (!element) return;
    let isPressed = false;

    const start = (e) => {
      e.preventDefault();
      if (!isPressed) {
        isPressed = true;
        element.classList.add('active');
        onDown();
      }
    };

    const end = (e) => {
      e.preventDefault();
      if (isPressed) {
        isPressed = false;
        element.classList.remove('active');
        onUp();
      }
    };

    element.addEventListener('touchstart', start, { passive: false });
    element.addEventListener('touchend', end, { passive: false });
    element.addEventListener('touchcancel', end, { passive: false });
    element.addEventListener('mousedown', start);
    window.addEventListener('mouseup', (e) => {
      if (isPressed) end(e);
    });
  }

  // --- Tastatur, Modifikatoren und Sondertasten ---
  function initKeyboards() {
    // Modifikatoren (Strg, Alt, Shift, Win)
    document.querySelectorAll('.mod-key').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        vibrate(15);
        const keyName = btn.dataset.key;
        toggleModifier(keyName);
      });
    });

    // Normale Tasten (Esc, Tab, Enter, Backspace, Pfeile, F-Tasten)
    document.querySelectorAll('.key-btn[data-key]').forEach((btn) => {
      if (btn.classList.contains('mod-key')) return;
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        vibrate(15);
        const keyName = btn.dataset.key;
        sendKeyPress(keyName);
      });
    });

    // Direkte Tastatur-Zeichen (data-char)
    document.querySelectorAll('.key-btn[data-char]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        vibrate(12);
        let char = btn.dataset.char;

        // Shift oder CapsLock aktiv?
        const isShift = state.modifiers.Shift || state.capsLock;
        if (isShift && germanShiftMap[char]) {
          char = germanShiftMap[char];
        } else if (state.capsLock && char.length === 1 && char.match(/[a-zäöü]/i)) {
          char = char.toUpperCase();
        }

        send({ type: 'TextInput', text: char });

        // Wenn normales Shift gedrückt war, danach wieder zurücksetzen
        if (state.modifiers.Shift) {
          toggleModifier('Shift', false);
        }
      });
    });

    // Tastenkombinationen (z. B. Alt+Tab, Win+D)
    document.querySelectorAll('.shortcut-btn[data-combo]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        vibrate(20);
        const combo = btn.dataset.combo;
        send({ type: 'KeyCombo', combo });
      });
    });

    // Media Keys
    document.querySelectorAll('.media-btn[data-media]').forEach((btn) => {
      btn.addEventListener('click', (e) => {
        e.preventDefault();
        vibrate(20);
        const media = btn.dataset.media;
        send({ type: 'MediaKey', key: media });
      });
    });

    // CapsLock Button
    if (el.capsLockBtn) {
      el.capsLockBtn.addEventListener('click', () => {
        vibrate(15);
        state.capsLock = !state.capsLock;
        el.capsLockBtn.classList.toggle('active', state.capsLock);
        updateKeyLabels();
      });
    }

    // Toggle Virtual Keyboard Drawer
    el.btnToggleKbd.addEventListener('click', () => {
      vibrate(15);
      el.keyboardDrawer.classList.toggle('hidden');
    });
    el.btnCloseKbd.addEventListener('click', () => {
      el.keyboardDrawer.classList.add('hidden');
    });

    // Native iOS Tastatur / Diktat Trigger
    el.btnNativeKbd.addEventListener('click', () => {
      vibrate(15);
      el.hiddenTextInput.focus();
    });

    el.hiddenTextInput.addEventListener('input', (e) => {
      const text = el.hiddenTextInput.value;
      if (text.length > 0) {
        send({ type: 'TextInput', text });
        el.hiddenTextInput.value = '';
      }
    });

    el.hiddenTextInput.addEventListener('keydown', (e) => {
      if (e.key === 'Backspace') {
        send({ type: 'KeyPress', key: 'Backspace' });
      } else if (e.key === 'Enter') {
        send({ type: 'KeyPress', key: 'Enter' });
      }
    });
  }

  function toggleModifier(keyName, forceState) {
    const newState = forceState !== undefined ? forceState : !state.modifiers[keyName];
    state.modifiers[keyName] = newState;

    // Send modifier status to Windows host
    send({
      type: newState ? 'KeyDown' : 'KeyUp',
      key: keyName,
    });

    // Visual button update
    document.querySelectorAll(`.mod-key[data-key="${keyName}"]`).forEach((b) => {
      b.classList.toggle('active', newState);
    });

    if (keyName === 'Shift') {
      updateKeyLabels();
    }
  }

  function sendKeyPress(keyName) {
    send({ type: 'KeyPress', key: keyName });
  }

  function updateKeyLabels() {
    const isShift = state.modifiers.Shift || state.capsLock;
    document.querySelectorAll('.key-btn[data-char]').forEach((btn) => {
      const baseChar = btn.dataset.char;
      if (isShift && germanShiftMap[baseChar]) {
        btn.textContent = germanShiftMap[baseChar];
      } else if (state.capsLock && baseChar.length === 1 && baseChar.match(/[a-zäöü]/i)) {
        btn.textContent = baseChar.toUpperCase();
      } else {
        btn.textContent = baseChar;
      }
    });
  }

  // --- Settings & Modal Logic ---
  function initSettings() {
    // Settings open/close
    el.btnSettings.addEventListener('click', () => {
      vibrate(15);
      el.settingsModal.classList.remove('hidden');
    });
    el.btnCloseSettings.addEventListener('click', () => {
      el.settingsModal.classList.add('hidden');
    });

    // Fullscreen toggle
    el.btnFullscreen.addEventListener('click', () => {
      vibrate(15);
      if (!document.fullscreenElement) {
        document.documentElement.requestFullscreen().catch(() => {});
      } else {
        document.exitFullscreen().catch(() => {});
      }
    });

    // Sliders
    el.sliderSensitivity.addEventListener('input', (e) => {
      state.sensitivity = parseFloat(e.target.value);
      el.labelSensitivity.textContent = `${state.sensitivity.toFixed(1)}x`;
      localStorage.setItem('remote_pc_sens', state.sensitivity);
    });

    el.sliderScrollSpeed.addEventListener('input', (e) => {
      state.scrollSpeed = parseFloat(e.target.value);
      el.labelScrollSpeed.textContent = `${state.scrollSpeed.toFixed(1)}x`;
      localStorage.setItem('remote_pc_scroll_speed', state.scrollSpeed);
    });

    el.checkNaturalScroll.addEventListener('change', (e) => {
      state.naturalScroll = e.target.checked;
      localStorage.setItem('remote_pc_natural_scroll', state.naturalScroll);
    });

    el.checkHaptics.addEventListener('change', (e) => {
      state.haptics = e.target.checked;
      localStorage.setItem('remote_pc_haptics', state.haptics);
    });

    // Saved settings restore
    const savedSens = localStorage.getItem('remote_pc_sens');
    if (savedSens) {
      state.sensitivity = parseFloat(savedSens);
      el.sliderSensitivity.value = state.sensitivity;
      el.labelSensitivity.textContent = `${state.sensitivity.toFixed(1)}x`;
    }

    const savedScroll = localStorage.getItem('remote_pc_scroll_speed');
    if (savedScroll) {
      state.scrollSpeed = parseFloat(savedScroll);
      el.sliderScrollSpeed.value = state.scrollSpeed;
      el.labelScrollSpeed.textContent = `${state.scrollSpeed.toFixed(1)}x`;
    }

    const savedNatural = localStorage.getItem('remote_pc_natural_scroll');
    if (savedNatural !== null) {
      state.naturalScroll = savedNatural === 'true';
      el.checkNaturalScroll.checked = state.naturalScroll;
    }

    const savedHaptics = localStorage.getItem('remote_pc_haptics');
    if (savedHaptics !== null) {
      state.haptics = savedHaptics === 'true';
      el.checkHaptics.checked = state.haptics;
    }

    // Token Speichern
    el.btnSaveToken.addEventListener('click', () => {
      const newToken = el.inputManualToken.value.trim();
      if (newToken) {
        state.token = newToken;
        localStorage.setItem('remote_pc_token', newToken);
        el.settingsModal.classList.add('hidden');
        connectWebSocket();
      }
    });

    // Pairing Modal Connect
    el.btnConnectPairing.addEventListener('click', () => {
      const enteredToken = el.inputPairingToken.value.trim();
      if (enteredToken) {
        state.token = enteredToken;
        localStorage.setItem('remote_pc_token', enteredToken);
        connectWebSocket();
      }
    });
  }

  // --- Initialize App ---
  function init() {
    initToken();
    initTrackpad();
    initMouseButtons();
    initKeyboards();
    initSettings();
    connectWebSocket();
  }

  // DOM ready
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }

})();
