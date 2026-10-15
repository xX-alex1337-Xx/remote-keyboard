#![windows_subsystem = "windows"]

use local_ip_address::local_ip;
use qrcode::QrCode;
use remote_keyboard_lib::{
    auth::AuthState,
    config::{is_autostart_registered, set_autostart_registered, AppConfig},
    server::{create_router, AppState},
};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{BOOL, COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_USE_IMMERSIVE_DARK_MODE,
    DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreatePen,
    CreateSolidBrush, DeleteDC, DeleteObject, DrawTextW, EndPaint, FillRect,
    FrameRect, InvalidateRect, LineTo, MoveToEx, RoundRect, ScreenToClient, SelectObject, SetBkMode,
    SetTextColor, BACKGROUND_MODE, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER, FW_BOLD,
    FW_NORMAL, FW_SEMIBOLD, HDC, PAINTSTRUCT, PS_SOLID, SRCCOPY,
};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAYICON: u32 = WM_USER + 101;
const WM_MOUSELEAVE: u32 = 0x02A3;

const ID_TRAY_SHOW_QR: usize = 2001;
const ID_TRAY_COPY_URL: usize = 2002;
const ID_TRAY_NEW_TOKEN: usize = 2003;
const ID_TRAY_AUTOSTART: usize = 2004;
const ID_TRAY_EXIT: usize = 2005;

const TIMER_COPY_RESET: usize = 9001;

const FLYOUT_WIDTH: i32 = 340;
const FLYOUT_HEIGHT: i32 = 475;

const CLOSE_BTN_RECT: RECT = RECT { left: 300, top: 16, right: 326, bottom: 42 };
const QR_CARD_RECT: RECT = RECT { left: 55, top: 60, right: 285, bottom: 290 };
const COPY_BTN_RECT: RECT = RECT { left: 45, top: 354, right: 295, bottom: 398 };
const TOKEN_LINK_RECT: RECT = RECT { left: 50, top: 416, right: 290, bottom: 440 };

#[allow(non_snake_case)]
#[repr(C)]
struct TRACKMOUSEEVENT {
    cbSize: u32,
    dwFlags: u32,
    hwndTrack: HWND,
    dwHoverTime: u32,
}
const TME_LEAVE: u32 = 0x00000002;

#[link(name = "user32")]
extern "system" {
    fn TrackMouseEvent(lpEventTrack: *mut TRACKMOUSEEVENT) -> BOOL;
}

#[derive(Default)]
struct FlyoutState {
    recently_copied: bool,
    hovered_close: bool,
    hovered_copy: bool,
    hovered_token: bool,
    hovered_qr: bool,
}

struct TrayAppContext {
    config: Arc<Mutex<AppConfig>>,
    auth: AuthState,
    nid: NOTIFYICONDATAW,
    #[allow(dead_code)]
    h_app_icon: HICON,
    h_tray_icon: HICON,
    flyout_state: Mutex<FlyoutState>,
}

unsafe impl Send for TrayAppContext {}
unsafe impl Sync for TrayAppContext {}

static APP_CTX: OnceLock<TrayAppContext> = OnceLock::new();

fn main() {
    if let Err(e) = run() {
        let dir = remote_keyboard_lib::config::get_config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("error.log"), format!("Fehler beim Starten: {}\n", e));
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Arc::new(Mutex::new(AppConfig::load_or_create()));
    let (port, token) = {
        let c = config.lock().unwrap();
        (c.port, c.token.clone())
    };

    let auth = AuthState::with_token(token);

    // Axum Webserver im Single-Thread Tokio-Hintergrund starten
    let auth_clone = auth.clone();
    std::thread::spawn(move || {
        let dir = remote_keyboard_lib::config::get_config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let log_file = dir.join("server.log");
        let _ = std::fs::write(&log_file, format!("Starte Background Server auf Port {}...\n", port));

        match tokio::runtime::Builder::new_current_thread().enable_all().build() {
            Ok(rt) => {
                rt.block_on(async move {
                    let app_state = AppState { auth: auth_clone };
                    let app = create_router(app_state);
                    let addr = SocketAddr::from(([0, 0, 0, 0], port));
                    match tokio::net::TcpListener::bind(addr).await {
                        Ok(listener) => {
                            let _ = std::fs::write(&log_file, format!("Server gebunden an {}\n", addr));
                            if let Err(e) = axum::serve(listener, app).await {
                                let _ = std::fs::write(dir.join("server_err.log"), format!("Server Error: {}\n", e));
                            }
                        }
                        Err(e) => {
                            let _ = std::fs::write(dir.join("server_err.log"), format!("Bind Error: {}\n", e));
                        }
                    }
                });
            }
            Err(e) => {
                let _ = std::fs::write(dir.join("server_err.log"), format!("Runtime Error: {}\n", e));
            }
        }
    });

    // Win32 GUI & Tray Initialisierung
    unsafe {
        let instance = HINSTANCE(GetModuleHandleW(None)?.0);

        // Icons laden (Ressource ID 1 aus app.rc oder Fallback)
        let h_app_icon = LoadImageW(
            instance,
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_SHARED,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap());

        let h_tray_icon = LoadImageW(
            instance,
            PCWSTR(1 as *const u16),
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        )
        .map(|h| HICON(h.0))
        .unwrap_or_else(|_| LoadIconW(None, IDI_APPLICATION).unwrap());

        // Fensterklasse für das Flyout-Fenster registrieren
        let class_name = w!("RemoteKeyboardFlyoutClass");
        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: CreateSolidBrush(COLORREF(0x00000000)), // OLED Schwarz
            lpszClassName: class_name,
            hIcon: h_app_icon,
            hIconSm: h_tray_icon,
            ..Default::default()
        };
        RegisterClassExW(&wnd_class);

        // Rahmenloses Popup-Fenster mit DWM-Rundung erstellen (initial versteckt)
        let flyout_hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Windows Remote Steuerung"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            FLYOUT_WIDTH,
            FLYOUT_HEIGHT,
            None,
            None,
            instance,
            None,
        )?;

        // Windows 11 DWM-Styling anwenden
        let corner_pref = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            flyout_hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner_pref as *const _ as _,
            std::mem::size_of::<windows::Win32::Graphics::Dwm::DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );

        let dark_mode = BOOL(1);
        let _ = DwmSetWindowAttribute(
            flyout_hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            &dark_mode as *const _ as _,
            std::mem::size_of::<BOOL>() as u32,
        );

        let border_color = COLORREF(0x002D2D2D);
        let _ = DwmSetWindowAttribute(
            flyout_hwnd,
            DWMWA_BORDER_COLOR,
            &border_color as *const _ as _,
            std::mem::size_of::<COLORREF>() as u32,
        );

        // System Tray Icon anlegen
        let mut tip: [u16; 128] = [0; 128];
        let tip_str = "Windows Remote Steuerung\0".encode_utf16();
        for (i, c) in tip_str.enumerate() {
            if i < 127 {
                tip[i] = c;
            }
        }

        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: flyout_hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: h_tray_icon,
            szTip: tip,
            ..Default::default()
        };

        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let _ = APP_CTX.set(TrayAppContext {
            config,
            auth,
            nid,
            h_app_icon,
            h_tray_icon,
            flyout_state: Mutex::new(FlyoutState::default()),
        });

        // Message Loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        // Aufräumen beim Beenden
        if let Some(ctx) = APP_CTX.get() {
            let _ = Shell_NotifyIconW(NIM_DELETE, &ctx.nid);
        }
    }

    Ok(())
}

fn get_current_access_url() -> String {
    if let Some(ctx) = APP_CTX.get() {
        let port = ctx.config.lock().unwrap().port;
        let token = ctx.auth.get_token();
        let ip = local_ip().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());
        format!("http://{}:{}/?token={}", ip, port, token)
    } else {
        "http://127.0.0.1:8080/".to_string()
    }
}

fn copy_to_clipboard(text: &str) {
    unsafe {
        if OpenClipboard(None).is_ok() {
            let _ = EmptyClipboard();
            let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
            let bytes_len = wide.len() * 2;
            if let Ok(h_mem) = GlobalAlloc(GHND, bytes_len) {
                let p_mem = GlobalLock(h_mem) as *mut u16;
                if !p_mem.is_null() {
                    std::ptr::copy_nonoverlapping(wide.as_ptr(), p_mem, wide.len());
                    let _ = GlobalUnlock(h_mem);
                    let _ = SetClipboardData(13, windows::Win32::Foundation::HANDLE(h_mem.0));
                }
            }
            let _ = CloseClipboard();
        }
    }
}

fn generate_new_token() {
    if let Some(ctx) = APP_CTX.get() {
        let new_token = remote_keyboard_lib::config::generate_random_token();
        ctx.auth.set_token(new_token.clone());
        let mut c = ctx.config.lock().unwrap();
        c.token = new_token;
        c.save();
    }
}

fn pt_in_rect(pt: POINT, rect: &RECT) -> bool {
    pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom
}

unsafe fn show_qr_flyout(hwnd: HWND) {
    let mut work_area = RECT::default();
    let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        Some(&mut work_area as *mut _ as *mut _),
        windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    );

    let win_w = FLYOUT_WIDTH;
    let win_h = FLYOUT_HEIGHT;
    let margin = 12;

    let x = work_area.right - win_w - margin;
    let y = work_area.bottom - win_h - margin;

    let _ = SetWindowPos(
        hwnd,
        HWND_TOPMOST,
        x,
        y,
        win_w,
        win_h,
        SWP_SHOWWINDOW,
    );
    let _ = SetForegroundWindow(hwnd);
    let _ = InvalidateRect(hwnd, None, false);
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            } else if event == WM_LBUTTONUP {
                if IsWindowVisible(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                } else {
                    show_qr_flyout(hwnd);
                }
            }
            LRESULT(0)
        }
        WM_ACTIVATE => {
            let active = (wparam.0 & 0xFFFF) as u32;
            if active == WA_INACTIVE {
                // Automatisches Schließen bei Klick außerhalb
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                // Schließen bei Escape
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = ScreenToClient(hwnd, &mut pt);

            if pt_in_rect(pt, &CLOSE_BTN_RECT)
                || pt_in_rect(pt, &COPY_BTN_RECT)
                || pt_in_rect(pt, &TOKEN_LINK_RECT)
                || pt_in_rect(pt, &QR_CARD_RECT)
            {
                let _ = SetCursor(LoadCursorW(None, IDC_HAND).unwrap());
                return LRESULT(1);
            } else {
                let _ = SetCursor(LoadCursorW(None, IDC_ARROW).unwrap());
                return LRESULT(1);
            }
        }
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let pt = POINT { x, y };

            let mut track = TRACKMOUSEEVENT {
                cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                dwFlags: TME_LEAVE,
                hwndTrack: hwnd,
                dwHoverTime: 0,
            };
            let _ = TrackMouseEvent(&mut track);

            if let Some(ctx) = APP_CTX.get() {
                let mut state = ctx.flyout_state.lock().unwrap();
                let h_close = pt_in_rect(pt, &CLOSE_BTN_RECT);
                let h_copy = pt_in_rect(pt, &COPY_BTN_RECT);
                let h_token = pt_in_rect(pt, &TOKEN_LINK_RECT);
                let h_qr = pt_in_rect(pt, &QR_CARD_RECT);

                if state.hovered_close != h_close
                    || state.hovered_copy != h_copy
                    || state.hovered_token != h_token
                    || state.hovered_qr != h_qr
                {
                    state.hovered_close = h_close;
                    state.hovered_copy = h_copy;
                    state.hovered_token = h_token;
                    state.hovered_qr = h_qr;
                    drop(state);
                    let _ = InvalidateRect(hwnd, None, false);
                }
            }
            LRESULT(0)
        }
        WM_MOUSELEAVE => {
            if let Some(ctx) = APP_CTX.get() {
                let mut state = ctx.flyout_state.lock().unwrap();
                state.hovered_close = false;
                state.hovered_copy = false;
                state.hovered_token = false;
                state.hovered_qr = false;
                drop(state);
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as i32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as i32;
            let pt = POINT { x, y };

            if pt_in_rect(pt, &CLOSE_BTN_RECT) {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else if pt_in_rect(pt, &COPY_BTN_RECT) || pt_in_rect(pt, &QR_CARD_RECT) {
                copy_to_clipboard(&get_current_access_url());
                if let Some(ctx) = APP_CTX.get() {
                    let mut state = ctx.flyout_state.lock().unwrap();
                    state.recently_copied = true;
                    let _ = SetTimer(hwnd, TIMER_COPY_RESET, 2500, None);
                }
                let _ = InvalidateRect(hwnd, None, false);
            } else if pt_in_rect(pt, &TOKEN_LINK_RECT) {
                generate_new_token();
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_TIMER => {
            if wparam.0 == TIMER_COPY_RESET {
                let _ = KillTimer(hwnd, TIMER_COPY_RESET);
                if let Some(ctx) = APP_CTX.get() {
                    let mut state = ctx.flyout_state.lock().unwrap();
                    state.recently_copied = false;
                }
                let _ = InvalidateRect(hwnd, None, false);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as usize;
            match id {
                ID_TRAY_SHOW_QR => {
                    show_qr_flyout(hwnd);
                }
                ID_TRAY_COPY_URL => {
                    copy_to_clipboard(&get_current_access_url());
                    if let Some(ctx) = APP_CTX.get() {
                        let mut state = ctx.flyout_state.lock().unwrap();
                        state.recently_copied = true;
                        let _ = SetTimer(hwnd, TIMER_COPY_RESET, 2500, None);
                    }
                }
                ID_TRAY_NEW_TOKEN => {
                    generate_new_token();
                    let _ = InvalidateRect(hwnd, None, false);
                }
                ID_TRAY_AUTOSTART => {
                    let current = is_autostart_registered();
                    let new_val = !current;
                    let _ = set_autostart_registered(new_val);
                    if let Some(ctx) = APP_CTX.get() {
                        let mut c = ctx.config.lock().unwrap();
                        c.autostart = new_val;
                        c.save();
                    }
                }
                ID_TRAY_EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            draw_flyout(hwnd, hdc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = ShowWindow(hwnd, SW_HIDE);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

unsafe fn show_tray_menu(hwnd: HWND) {
    let mut pt = POINT::default();
    let _ = GetCursorPos(&mut pt);

    if let Ok(menu) = CreatePopupMenu() {
        let is_auto = is_autostart_registered();
        let auto_flag = if is_auto { MF_CHECKED } else { MF_UNCHECKED };

        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_SHOW_QR, w!("QR-Code Flyout anzeigen"));
        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_COPY_URL, w!("Verbindungs-URL kopieren"));
        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_NEW_TOKEN, w!("Neuen Token generieren"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING | auto_flag, ID_TRAY_AUTOSTART, w!("Mit Windows starten"));
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_EXIT, w!("Beenden"));

        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            0,
            hwnd,
            None,
        );
        let _ = DestroyMenu(menu);
    }
}

unsafe fn draw_flyout(hwnd: HWND, hdc: HDC) {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);
    let w = client_rect.right - client_rect.left;
    let h = client_rect.bottom - client_rect.top;

    // Double-Buffering gegen Flackern
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
    let old_bmp = SelectObject(mem_dc, mem_bmp);

    // 1. OLED Schwarz Hintergrund
    let bg_brush = CreateSolidBrush(COLORREF(0x00000000));
    FillRect(mem_dc, &client_rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // 2. Feiner 1px Rand (#2D2D2D)
    let border_brush = CreateSolidBrush(COLORREF(0x002D2D2D));
    FrameRect(mem_dc, &client_rect, border_brush);
    let _ = DeleteObject(border_brush);

    SetBkMode(mem_dc, BACKGROUND_MODE(1)); // TRANSPARENT

    let (hover_close, hover_copy, hover_token, recently_copied) = if let Some(ctx) = APP_CTX.get() {
        let s = ctx.flyout_state.lock().unwrap();
        (s.hovered_close, s.hovered_copy, s.hovered_token, s.recently_copied)
    } else {
        (false, false, false, false)
    };

    // 3. Header: Icon + Titel + Schließen-Kreuz
    if let Some(ctx) = APP_CTX.get() {
        let _ = DrawIconEx(mem_dc, 20, 18, ctx.h_tray_icon, 20, 20, 0, None, DI_NORMAL);
    }

    let font_header = CreateFontW(
        16, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI Variable Text"),
    );
    let prev_font = SelectObject(mem_dc, font_header);
    SetTextColor(mem_dc, COLORREF(0x00FFFFFF));

    let mut header_rect = RECT { left: 48, top: 16, right: 295, bottom: 40 };
    let mut header_text: Vec<u16> = "Windows Remote Steuerung".encode_utf16().collect();
    DrawTextW(mem_dc, &mut header_text, &mut header_rect, DT_SINGLELINE | DT_VCENTER);

    // Schließen-Button (✕)
    if hover_close {
        let close_brush = CreateSolidBrush(COLORREF(0x00262626));
        let close_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00262626));
        let old_brush = SelectObject(mem_dc, close_brush);
        let old_pen = SelectObject(mem_dc, close_pen);
        let _ = RoundRect(mem_dc, CLOSE_BTN_RECT.left, CLOSE_BTN_RECT.top, CLOSE_BTN_RECT.right, CLOSE_BTN_RECT.bottom, 6, 6);
        SelectObject(mem_dc, old_brush);
        SelectObject(mem_dc, old_pen);
        let _ = DeleteObject(close_brush);
        let _ = DeleteObject(close_pen);
    }

    SetTextColor(mem_dc, if hover_close { COLORREF(0x00FFFFFF) } else { COLORREF(0x008A8A8A) });
    let mut close_rect = CLOSE_BTN_RECT;
    let mut close_text: Vec<u16> = "\u{2715}".encode_utf16().collect();
    DrawTextW(mem_dc, &mut close_text, &mut close_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

    // Trennlinie unter dem Header
    let sep_pen = CreatePen(PS_SOLID, 1, COLORREF(0x001C1C1C));
    let old_pen = SelectObject(mem_dc, sep_pen);
    let _ = MoveToEx(mem_dc, 20, 48, None);
    let _ = LineTo(mem_dc, 320, 48);
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(sep_pen);

    // 4. Zentraler QR-Code auf abgerundeter weißer Karte
    let card_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
    let card_pen = CreatePen(PS_SOLID, 1, COLORREF(0x00E4E4E4));
    let old_brush = SelectObject(mem_dc, card_brush);
    let old_pen = SelectObject(mem_dc, card_pen);
    let _ = RoundRect(mem_dc, QR_CARD_RECT.left, QR_CARD_RECT.top, QR_CARD_RECT.right, QR_CARD_RECT.bottom, 16, 16);
    SelectObject(mem_dc, old_brush);
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(card_brush);
    let _ = DeleteObject(card_pen);

    let access_url = get_current_access_url();
    if let Ok(code) = QrCode::new(access_url.as_bytes()) {
        let width = code.width() as i32;
        let pixel_scale = 6;
        let qr_pixel_size = width * pixel_scale;
        let card_w = QR_CARD_RECT.right - QR_CARD_RECT.left;
        let card_h = QR_CARD_RECT.bottom - QR_CARD_RECT.top;
        let qr_start_x = QR_CARD_RECT.left + (card_w - qr_pixel_size) / 2;
        let qr_start_y = QR_CARD_RECT.top + (card_h - qr_pixel_size) / 2;

        let black_brush = CreateSolidBrush(COLORREF(0x00000000));
        for y in 0..width {
            for x in 0..width {
                if code[(x as usize, y as usize)] == qrcode::Color::Dark {
                    let rx = qr_start_x + x * pixel_scale;
                    let ry = qr_start_y + y * pixel_scale;
                    let module_rect = RECT {
                        left: rx,
                        top: ry,
                        right: rx + pixel_scale,
                        bottom: ry + pixel_scale,
                    };
                    FillRect(mem_dc, &module_rect, black_brush);
                }
            }
        }
        let _ = DeleteObject(black_brush);
    }

    // 5. Texte unter dem QR-Code
    let font_prompt = CreateFontW(
        15, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI Variable Display"),
    );
    SelectObject(mem_dc, font_prompt);
    SetTextColor(mem_dc, COLORREF(0x00FFFFFF));

    let mut prompt_rect = RECT { left: 20, top: 302, right: 320, bottom: 322 };
    let mut prompt_text: Vec<u16> = "Mit iPad / iPhone scannen".encode_utf16().collect();
    DrawTextW(mem_dc, &mut prompt_text, &mut prompt_rect, DT_CENTER | DT_SINGLELINE);

    let font_url = CreateFontW(
        11, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI Variable Text"),
    );
    SelectObject(mem_dc, font_url);
    SetTextColor(mem_dc, COLORREF(0x00808080));

    let mut url_rect = RECT { left: 20, top: 324, right: 320, bottom: 342 };
    let mut url_text: Vec<u16> = access_url.encode_utf16().collect();
    DrawTextW(mem_dc, &mut url_text, &mut url_rect, DT_CENTER | DT_SINGLELINE | DT_NOPREFIX);

    // 6. Moderner "URL kopieren" Button
    let btn_bg = if recently_copied {
        COLORREF(0x001B2E1C) // Dunkelgrün
    } else if hover_copy {
        COLORREF(0x00262626) // Hover dunkelgrau
    } else {
        COLORREF(0x001A1A1A) // Normal dunkelgrau
    };

    let btn_border = if recently_copied {
        COLORREF(0x00337A38)
    } else if hover_copy {
        COLORREF(0x003D3D3D)
    } else {
        COLORREF(0x002C2C2C)
    };

    let btn_brush = CreateSolidBrush(btn_bg);
    let btn_pen = CreatePen(PS_SOLID, 1, btn_border);
    let old_brush = SelectObject(mem_dc, btn_brush);
    let old_pen = SelectObject(mem_dc, btn_pen);
    let _ = RoundRect(mem_dc, COPY_BTN_RECT.left, COPY_BTN_RECT.top, COPY_BTN_RECT.right, COPY_BTN_RECT.bottom, 10, 10);
    SelectObject(mem_dc, old_brush);
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(btn_brush);
    let _ = DeleteObject(btn_pen);

    let font_btn = CreateFontW(
        13, 0, 0, 0, FW_SEMIBOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI Variable Text"),
    );
    SelectObject(mem_dc, font_btn);

    if recently_copied {
        SetTextColor(mem_dc, COLORREF(0x0078D984)); // Frisches Grün
        let mut btn_rect = COPY_BTN_RECT;
        let mut btn_text: Vec<u16> = "\u{2713} In Zwischenablage kopiert!".encode_utf16().collect();
        DrawTextW(mem_dc, &mut btn_text, &mut btn_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    } else {
        SetTextColor(mem_dc, COLORREF(0x00FFFFFF));
        let mut btn_rect = COPY_BTN_RECT;
        let mut btn_text: Vec<u16> = "Verbindungs-URL kopieren".encode_utf16().collect();
        DrawTextW(mem_dc, &mut btn_text, &mut btn_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    }

    // 7. Footer: "Neuen Token generieren" Link
    let font_link = CreateFontW(
        11, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI Variable Text"),
    );
    SelectObject(mem_dc, font_link);
    SetTextColor(mem_dc, if hover_token { COLORREF(0x009AE0FF) } else { COLORREF(0x0052C3FF) });

    let mut token_rect = TOKEN_LINK_RECT;
    let mut token_text: Vec<u16> = "Neuen Sicherheitstoken generieren".encode_utf16().collect();
    DrawTextW(mem_dc, &mut token_text, &mut token_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

    // Blit auf das eigentliche Fenster
    let _ = BitBlt(hdc, 0, 0, w, h, mem_dc, 0, 0, SRCCOPY);

    // Ressourcen aufräumen
    SelectObject(mem_dc, prev_font);
    SelectObject(mem_dc, old_bmp);
    let _ = DeleteObject(font_header);
    let _ = DeleteObject(font_prompt);
    let _ = DeleteObject(font_url);
    let _ = DeleteObject(font_btn);
    let _ = DeleteObject(font_link);
    let _ = DeleteObject(mem_bmp);
    let _ = DeleteDC(mem_dc);
}
