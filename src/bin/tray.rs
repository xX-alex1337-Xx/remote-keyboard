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
    SetTextColor, BACKGROUND_MODE, DT_CENTER, DT_NOPREFIX, DT_SINGLELINE, DT_VCENTER,
    FW_NORMAL, FW_SEMIBOLD, HDC, HFONT, PAINTSTRUCT, PS_SOLID, SRCCOPY,
};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
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

// Grayscale antialiasing for pure OLED black (no subpixel color fringing)
const ANTIALIASED_QUALITY: u32 = 4;

const fn rgb(r: u8, g: u8, b: u8) -> COLORREF {
    COLORREF((r as u32) | ((g as u32) << 8) | ((b as u32) << 16))
}

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

struct FlyoutLayout {
    w: i32,
    h: i32,
    dpi: u32,
    close_rect: RECT,
    qr_card_rect: RECT,
    copy_btn_rect: RECT,
    token_link_rect: RECT,
}

fn get_window_dpi(hwnd: HWND) -> u32 {
    unsafe {
        let d = GetDpiForWindow(hwnd);
        if d > 0 { d } else { 96 }
    }
}

fn get_flyout_layout(hwnd: HWND) -> FlyoutLayout {
    let dpi = get_window_dpi(hwnd);
    let s = |v: i32| -> i32 { ((v as f32 * dpi as f32) / 96.0).round() as i32 };

    let mut client_rect = RECT::default();
    unsafe {
        let _ = GetClientRect(hwnd, &mut client_rect);
    }
    let w = if client_rect.right > 0 { client_rect.right } else { s(340) };
    let h = if client_rect.bottom > 0 { client_rect.bottom } else { s(480) };

    let card_w = w - s(80);
    let card_top = s(60);
    let card_bottom = card_top + card_w;

    FlyoutLayout {
        w,
        h,
        dpi,
        close_rect: RECT {
            left: w - s(38),
            top: s(16),
            right: w - s(14),
            bottom: s(40),
        },
        qr_card_rect: RECT {
            left: s(40),
            top: card_top,
            right: w - s(40),
            bottom: card_bottom,
        },
        copy_btn_rect: RECT {
            left: s(36),
            top: card_bottom + s(68),
            right: w - s(36),
            bottom: card_bottom + s(112),
        },
        token_link_rect: RECT {
            left: s(36),
            top: card_bottom + s(124),
            right: w - s(36),
            bottom: card_bottom + s(148),
        },
    }
}

unsafe fn create_fluent_font(pt: i32, weight: i32, dpi: u32) -> HFONT {
    // pt in Points (1 pt = 1/72 inch). In pixels: -((pt * dpi) / 72)
    let height = -((pt * dpi as i32) / 72);
    CreateFontW(
        height,
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        0,
        0,
        0,
        ANTIALIASED_QUALITY,
        0,
        w!("Segoe UI Variable Text"),
    )
}

fn pt_in_rect(pt: POINT, rect: &RECT) -> bool {
    pt.x >= rect.left && pt.x <= rect.right && pt.y >= rect.top && pt.y <= rect.bottom
}

fn main() {
    if let Err(e) = run() {
        let dir = remote_keyboard_lib::config::get_config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("error.log"), format!("Fehler beim Starten: {}\n", e));
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Per-Monitor V2 DPI-Awareness aktivieren (verhindert DWM Bitmap-Skalierung!)
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }

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

        // Icons laden mit nativer DPI-Größe aus app.rc
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

        // Rahmenloses Popup-Fenster mit DWM-Rundung erstellen
        let flyout_hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            class_name,
            w!("Windows Remote Steuerung"),
            WS_POPUP,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            340,
            480,
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

        let border_color = rgb(45, 45, 45);
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

unsafe fn show_qr_flyout(hwnd: HWND) {
    let dpi = get_window_dpi(hwnd);
    let s = |v: i32| -> i32 { ((v as f32 * dpi as f32) / 96.0).round() as i32 };

    let mut work_area = RECT::default();
    let _ = windows::Win32::UI::WindowsAndMessaging::SystemParametersInfoW(
        SPI_GETWORKAREA,
        0,
        Some(&mut work_area as *mut _ as *mut _),
        windows::Win32::UI::WindowsAndMessaging::SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
    );

    let card_w = s(340) - s(80);
    let win_w = s(340);
    let win_h = s(60) + card_w + s(160);
    let margin = s(12);

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
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_KEYDOWN => {
            if wparam.0 == VK_ESCAPE.0 as usize {
                let _ = ShowWindow(hwnd, SW_HIDE);
            }
            LRESULT(0)
        }
        WM_SETCURSOR => {
            let mut pt = POINT::default();
            let _ = GetCursorPos(&mut pt);
            let _ = ScreenToClient(hwnd, &mut pt);

            let layout = get_flyout_layout(hwnd);

            if pt_in_rect(pt, &layout.close_rect)
                || pt_in_rect(pt, &layout.copy_btn_rect)
                || pt_in_rect(pt, &layout.token_link_rect)
                || pt_in_rect(pt, &layout.qr_card_rect)
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
                let layout = get_flyout_layout(hwnd);
                let mut state = ctx.flyout_state.lock().unwrap();
                let h_close = pt_in_rect(pt, &layout.close_rect);
                let h_copy = pt_in_rect(pt, &layout.copy_btn_rect);
                let h_token = pt_in_rect(pt, &layout.token_link_rect);
                let h_qr = pt_in_rect(pt, &layout.qr_card_rect);

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

            let layout = get_flyout_layout(hwnd);

            if pt_in_rect(pt, &layout.close_rect) {
                let _ = ShowWindow(hwnd, SW_HIDE);
            } else if pt_in_rect(pt, &layout.copy_btn_rect) || pt_in_rect(pt, &layout.qr_card_rect) {
                copy_to_clipboard(&get_current_access_url());
                if let Some(ctx) = APP_CTX.get() {
                    let mut state = ctx.flyout_state.lock().unwrap();
                    state.recently_copied = true;
                    let _ = SetTimer(hwnd, TIMER_COPY_RESET, 2500, None);
                }
                let _ = InvalidateRect(hwnd, None, false);
            } else if pt_in_rect(pt, &layout.token_link_rect) {
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
    let layout = get_flyout_layout(hwnd);
    let w = layout.w;
    let h = layout.h;
    let dpi = layout.dpi;
    let s = |v: i32| -> i32 { ((v as f32 * dpi as f32) / 96.0).round() as i32 };

    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    // Double-Buffering gegen Flackern
    let mem_dc = CreateCompatibleDC(hdc);
    let mem_bmp = CreateCompatibleBitmap(hdc, w, h);
    let old_bmp = SelectObject(mem_dc, mem_bmp);

    // 1. OLED Schwarz Hintergrund
    let bg_brush = CreateSolidBrush(rgb(0, 0, 0));
    FillRect(mem_dc, &client_rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    // 2. Feiner 1px Rand (#2D2D2D)
    let border_brush = CreateSolidBrush(rgb(45, 45, 45));
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
    let icon_size = GetSystemMetrics(SM_CXSMICON);
    let icon_y = s(16) + (s(24) - icon_size).max(0) / 2;
    if let Some(ctx) = APP_CTX.get() {
        let _ = DrawIconEx(mem_dc, s(18), icon_y, ctx.h_tray_icon, icon_size, icon_size, 0, None, DI_NORMAL);
    }

    let font_header = create_fluent_font(10, FW_SEMIBOLD.0 as i32, dpi);
    let prev_font = SelectObject(mem_dc, font_header);
    SetTextColor(mem_dc, rgb(255, 255, 255));

    let mut header_rect = RECT {
        left: s(18) + icon_size + s(8),
        top: s(16),
        right: layout.close_rect.left - s(6),
        bottom: s(40),
    };
    let mut header_text: Vec<u16> = "Windows Remote Steuerung".encode_utf16().collect();
    DrawTextW(mem_dc, &mut header_text, &mut header_rect, DT_SINGLELINE | DT_VCENTER);

    // Schließen-Button (✕)
    if hover_close {
        let close_brush = CreateSolidBrush(rgb(38, 38, 38));
        let close_pen = CreatePen(PS_SOLID, 1, rgb(38, 38, 38));
        let old_brush = SelectObject(mem_dc, close_brush);
        let old_pen = SelectObject(mem_dc, close_pen);
        let _ = RoundRect(
            mem_dc,
            layout.close_rect.left,
            layout.close_rect.top,
            layout.close_rect.right,
            layout.close_rect.bottom,
            s(6),
            s(6),
        );
        SelectObject(mem_dc, old_brush);
        SelectObject(mem_dc, old_pen);
        let _ = DeleteObject(close_brush);
        let _ = DeleteObject(close_pen);
    }

    SetTextColor(mem_dc, if hover_close { rgb(255, 255, 255) } else { rgb(140, 140, 140) });
    let mut close_rect = layout.close_rect;
    let mut close_text: Vec<u16> = "\u{2715}".encode_utf16().collect();
    DrawTextW(mem_dc, &mut close_text, &mut close_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

    // Trennlinie unter dem Header
    let sep_pen = CreatePen(PS_SOLID, 1, rgb(30, 30, 30));
    let old_pen = SelectObject(mem_dc, sep_pen);
    let _ = MoveToEx(mem_dc, s(18), s(48), None);
    let _ = LineTo(mem_dc, w - s(18), s(48));
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(sep_pen);

    // 4. Zentraler QR-Code auf abgerundeter weißer Karte
    let card_brush = CreateSolidBrush(rgb(255, 255, 255));
    let card_pen = CreatePen(PS_SOLID, 1, rgb(228, 228, 228));
    let old_brush = SelectObject(mem_dc, card_brush);
    let old_pen = SelectObject(mem_dc, card_pen);
    let _ = RoundRect(
        mem_dc,
        layout.qr_card_rect.left,
        layout.qr_card_rect.top,
        layout.qr_card_rect.right,
        layout.qr_card_rect.bottom,
        s(14),
        s(14),
    );
    SelectObject(mem_dc, old_brush);
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(card_brush);
    let _ = DeleteObject(card_pen);

    let access_url = get_current_access_url();
    if let Ok(code) = QrCode::new(access_url.as_bytes()) {
        let width = code.width() as i32;
        let card_w = layout.qr_card_rect.right - layout.qr_card_rect.left;
        let card_h = layout.qr_card_rect.bottom - layout.qr_card_rect.top;
        
        let avail = card_w - s(28);
        let pixel_scale = (avail / width).max(1);
        let qr_pixel_size = width * pixel_scale;
        let qr_start_x = layout.qr_card_rect.left + (card_w - qr_pixel_size) / 2;
        let qr_start_y = layout.qr_card_rect.top + (card_h - qr_pixel_size) / 2;

        let black_brush = CreateSolidBrush(rgb(0, 0, 0));
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
    let card_bottom = layout.qr_card_rect.bottom;

    let font_prompt = create_fluent_font(10, FW_SEMIBOLD.0 as i32, dpi);
    SelectObject(mem_dc, font_prompt);
    SetTextColor(mem_dc, rgb(255, 255, 255));

    let mut prompt_rect = RECT {
        left: s(18),
        top: card_bottom + s(14),
        right: w - s(18),
        bottom: card_bottom + s(38),
    };
    let mut prompt_text: Vec<u16> = "Mit iPad / iPhone scannen".encode_utf16().collect();
    DrawTextW(mem_dc, &mut prompt_text, &mut prompt_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);

    let font_url = create_fluent_font(8, FW_NORMAL.0 as i32, dpi);
    SelectObject(mem_dc, font_url);
    SetTextColor(mem_dc, rgb(155, 155, 155));

    let mut url_rect = RECT {
        left: s(18),
        top: card_bottom + s(40),
        right: w - s(18),
        bottom: card_bottom + s(60),
    };
    let mut url_text: Vec<u16> = access_url.encode_utf16().collect();
    DrawTextW(mem_dc, &mut url_text, &mut url_rect, DT_CENTER | DT_SINGLELINE | DT_NOPREFIX | DT_VCENTER);

    // 6. Moderner "URL kopieren" Button
    let btn_bg = if recently_copied {
        rgb(22, 46, 28) // Dunkelgrün
    } else if hover_copy {
        rgb(38, 38, 38) // Hover dunkelgrau
    } else {
        rgb(26, 26, 26) // Normal dunkelgrau
    };

    let btn_border = if recently_copied {
        rgb(46, 125, 50)
    } else if hover_copy {
        rgb(60, 60, 60)
    } else {
        rgb(44, 44, 44)
    };

    let btn_brush = CreateSolidBrush(btn_bg);
    let btn_pen = CreatePen(PS_SOLID, 1, btn_border);
    let old_brush = SelectObject(mem_dc, btn_brush);
    let old_pen = SelectObject(mem_dc, btn_pen);
    let _ = RoundRect(
        mem_dc,
        layout.copy_btn_rect.left,
        layout.copy_btn_rect.top,
        layout.copy_btn_rect.right,
        layout.copy_btn_rect.bottom,
        s(8),
        s(8),
    );
    SelectObject(mem_dc, old_brush);
    SelectObject(mem_dc, old_pen);
    let _ = DeleteObject(btn_brush);
    let _ = DeleteObject(btn_pen);

    let font_btn = create_fluent_font(9, FW_SEMIBOLD.0 as i32, dpi);
    SelectObject(mem_dc, font_btn);

    if recently_copied {
        SetTextColor(mem_dc, rgb(117, 214, 133)); // Frisches Fluent Grün
        let mut btn_rect = layout.copy_btn_rect;
        let mut btn_text: Vec<u16> = "\u{2713} In Zwischenablage kopiert!".encode_utf16().collect();
        DrawTextW(mem_dc, &mut btn_text, &mut btn_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    } else {
        SetTextColor(mem_dc, rgb(255, 255, 255));
        let mut btn_rect = layout.copy_btn_rect;
        let mut btn_text: Vec<u16> = "Verbindungs-URL kopieren".encode_utf16().collect();
        DrawTextW(mem_dc, &mut btn_text, &mut btn_rect, DT_CENTER | DT_SINGLELINE | DT_VCENTER);
    }

    // 7. Footer: "Neuen Token generieren" Link
    let font_link = create_fluent_font(8, FW_NORMAL.0 as i32, dpi);
    SelectObject(mem_dc, font_link);
    SetTextColor(mem_dc, if hover_token { rgb(128, 216, 255) } else { rgb(76, 194, 255) });

    let mut token_rect = layout.token_link_rect;
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
