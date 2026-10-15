use serde::Deserialize;
use tracing::warn;

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMessage {
    MouseMove { dx: i32, dy: i32 },
    MouseClick { button: MouseButton },
    MouseDown { button: MouseButton },
    MouseUp { button: MouseButton },
    MouseWheel { dx: i32, dy: i32 },
    KeyPress { key: String },
    KeyDown { key: String },
    KeyUp { key: String },
    TextInput { text: String },
    KeyCombo { combo: String },
    MediaKey { key: String },
    Ping,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

pub fn handle_client_message(msg: ClientMessage) {
    #[cfg(windows)]
    windows_impl::handle(msg);

    #[cfg(not(windows))]
    non_windows_impl::handle(msg);
}

#[cfg(windows)]
mod windows_impl {
    use super::*;
    use std::mem::size_of;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    pub fn handle(msg: ClientMessage) {
        match msg {
            ClientMessage::MouseMove { dx, dy } => {
                send_mouse_move(dx, dy);
            }
            ClientMessage::MouseClick { button } => {
                send_mouse_button(button, true);
                send_mouse_button(button, false);
            }
            ClientMessage::MouseDown { button } => {
                send_mouse_button(button, true);
            }
            ClientMessage::MouseUp { button } => {
                send_mouse_button(button, false);
            }
            ClientMessage::MouseWheel { dx, dy } => {
                if dy != 0 {
                    send_mouse_wheel(dy, false);
                }
                if dx != 0 {
                    send_mouse_wheel(dx, true);
                }
            }
            ClientMessage::KeyPress { key } => {
                if let Some(vk) = map_key_to_vk(&key) {
                    send_keyboard_vk(vk, true);
                    send_keyboard_vk(vk, false);
                } else if key.chars().count() == 1 {
                    send_text_unicode(&key);
                }
            }
            ClientMessage::KeyDown { key } => {
                if let Some(vk) = map_key_to_vk(&key) {
                    send_keyboard_vk(vk, true);
                }
            }
            ClientMessage::KeyUp { key } => {
                if let Some(vk) = map_key_to_vk(&key) {
                    send_keyboard_vk(vk, false);
                }
            }
            ClientMessage::TextInput { text } => {
                send_text_unicode(&text);
            }
            ClientMessage::KeyCombo { combo } => {
                execute_key_combo(&combo);
            }
            ClientMessage::MediaKey { key } => {
                if let Some(vk) = map_media_to_vk(&key) {
                    send_keyboard_vk(vk, true);
                    send_keyboard_vk(vk, false);
                }
            }
            ClientMessage::Ping => {}
        }
    }

    fn send_mouse_move(dx: i32, dy: i32) {
        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: 0,
                    dwFlags: MOUSEEVENTF_MOVE,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    fn send_mouse_button(button: MouseButton, is_down: bool) {
        let flags = match (button, is_down) {
            (MouseButton::Left, true) => MOUSEEVENTF_LEFTDOWN,
            (MouseButton::Left, false) => MOUSEEVENTF_LEFTUP,
            (MouseButton::Right, true) => MOUSEEVENTF_RIGHTDOWN,
            (MouseButton::Right, false) => MOUSEEVENTF_RIGHTUP,
            (MouseButton::Middle, true) => MOUSEEVENTF_MIDDLEDOWN,
            (MouseButton::Middle, false) => MOUSEEVENTF_MIDDLEUP,
        };

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    fn send_mouse_wheel(delta: i32, horizontal: bool) {
        let (flags, data) = if horizontal {
            (MOUSEEVENTF_HWHEEL, delta as u32)
        } else {
            (MOUSEEVENTF_WHEEL, delta as u32)
        };

        let input = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 {
                mi: MOUSEINPUT {
                    dx: 0,
                    dy: 0,
                    mouseData: data,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    fn send_keyboard_vk(vk: VIRTUAL_KEY, is_down: bool) {
        let flags = if is_down {
            KEYBD_EVENT_FLAGS(0)
        } else {
            KEYEVENTF_KEYUP
        };

        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        unsafe {
            SendInput(&[input], size_of::<INPUT>() as i32);
        }
    }

    fn send_text_unicode(text: &str) {
        let mut inputs = Vec::with_capacity(text.encode_utf16().count() * 2);
        for utf16 in text.encode_utf16() {
            // KeyDown Unicode
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: utf16,
                        dwFlags: KEYEVENTF_UNICODE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });
            // KeyUp Unicode
            inputs.push(INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VIRTUAL_KEY(0),
                        wScan: utf16,
                        dwFlags: KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            });
        }

        if !inputs.is_empty() {
            unsafe {
                SendInput(&inputs, size_of::<INPUT>() as i32);
            }
        }
    }

    fn execute_key_combo(combo: &str) {
        match combo {
            "Alt+Tab" => {
                send_keyboard_vk(VK_MENU, true);
                send_keyboard_vk(VK_TAB, true);
                send_keyboard_vk(VK_TAB, false);
                send_keyboard_vk(VK_MENU, false);
            }
            "Meta+d" => {
                send_keyboard_vk(VK_LWIN, true);
                send_keyboard_vk(VIRTUAL_KEY(b'D' as u16), true);
                send_keyboard_vk(VIRTUAL_KEY(b'D' as u16), false);
                send_keyboard_vk(VK_LWIN, false);
            }
            "Meta+e" => {
                send_keyboard_vk(VK_LWIN, true);
                send_keyboard_vk(VIRTUAL_KEY(b'E' as u16), true);
                send_keyboard_vk(VIRTUAL_KEY(b'E' as u16), false);
                send_keyboard_vk(VK_LWIN, false);
            }
            "Meta+l" => {
                send_keyboard_vk(VK_LWIN, true);
                send_keyboard_vk(VIRTUAL_KEY(b'L' as u16), true);
                send_keyboard_vk(VIRTUAL_KEY(b'L' as u16), false);
                send_keyboard_vk(VK_LWIN, false);
            }
            _ => {
                warn!("Unbekannte Tastenkombination: {}", combo);
            }
        }
    }

    fn map_key_to_vk(key: &str) -> Option<VIRTUAL_KEY> {
        match key {
            "Escape" => Some(VK_ESCAPE),
            "Tab" => Some(VK_TAB),
            "Enter" => Some(VK_RETURN),
            "Backspace" => Some(VK_BACK),
            "Delete" => Some(VK_DELETE),
            "CapsLock" => Some(VK_CAPITAL),
            "Shift" => Some(VK_SHIFT),
            "Control" => Some(VK_CONTROL),
            "Alt" => Some(VK_MENU),
            "AltGraph" => Some(VK_RMENU),
            "Meta" => Some(VK_LWIN),
            "ArrowUp" => Some(VK_UP),
            "ArrowDown" => Some(VK_DOWN),
            "ArrowLeft" => Some(VK_LEFT),
            "ArrowRight" => Some(VK_RIGHT),
            "F1" => Some(VK_F1),
            "F2" => Some(VK_F2),
            "F3" => Some(VK_F3),
            "F4" => Some(VK_F4),
            "F5" => Some(VK_F5),
            "F6" => Some(VK_F6),
            "F7" => Some(VK_F7),
            "F8" => Some(VK_F8),
            "F9" => Some(VK_F9),
            "F10" => Some(VK_F10),
            "F11" => Some(VK_F11),
            "F12" => Some(VK_F12),
            _ => None,
        }
    }

    fn map_media_to_vk(media: &str) -> Option<VIRTUAL_KEY> {
        match media {
            "VolumeMute" => Some(VK_VOLUME_MUTE),
            "VolumeDown" => Some(VK_VOLUME_DOWN),
            "VolumeUp" => Some(VK_VOLUME_UP),
            "MediaPlayPause" => Some(VK_MEDIA_PLAY_PAUSE),
            "MediaNextTrack" => Some(VK_MEDIA_NEXT_TRACK),
            "MediaPrevTrack" => Some(VK_MEDIA_PREV_TRACK),
            _ => None,
        }
    }
}

#[cfg(not(windows))]
mod non_windows_impl {
    use super::*;

    pub fn handle(msg: ClientMessage) {
        debug!("Nicht-Windows Dummy Empfangen: {:?}", msg);
    }
}
