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
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CreateFontW, CreateSolidBrush, DeleteObject, DrawTextW, EndPaint, FillRect,
    InvalidateRect, SelectObject, SetBkMode, SetTextColor, BACKGROUND_MODE, DT_CENTER, DT_NOPREFIX,
    DT_SINGLELINE, DT_WORDBREAK, FW_BOLD, FW_NORMAL, HDC, PAINTSTRUCT,
};
use windows::Win32::System::DataExchange::{CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GHND};
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAYICON: u32 = WM_USER + 101;
const ID_TRAY_SHOW_QR: usize = 2001;
const ID_TRAY_COPY_URL: usize = 2002;
const ID_TRAY_NEW_TOKEN: usize = 2003;
const ID_TRAY_AUTOSTART: usize = 2004;
const ID_TRAY_EXIT: usize = 2005;
const ID_BTN_COPY: usize = 3001;
const ID_BTN_CLOSE: usize = 3002;

struct TrayAppContext {
    config: Arc<Mutex<AppConfig>>,
    auth: AuthState,
    nid: NOTIFYICONDATAW,
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
    // Konfiguration laden
    let config = Arc::new(Mutex::new(AppConfig::load_or_create()));
    let (port, token) = {
        let c = config.lock().unwrap();
        (c.port, c.token.clone())
    };

    let auth = AuthState::with_token(token);

    // Axum Webserver im Hintergrund starten
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

        // Fensterklasse für das QR-Code Dialogfenster registrieren
        let class_name = w!("RemoteKeyboardTrayClass");
        let wnd_class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: instance,
            hCursor: LoadCursorW(None, IDC_ARROW)?,
            hbrBackground: CreateSolidBrush(COLORREF(0x00000000)), // OLED Schwarz
            lpszClassName: class_name,
            hIcon: LoadIconW(None, IDI_APPLICATION)?,
            ..Default::default()
        };
        RegisterClassExW(&wnd_class);

        // Dialog-Fenster erstellen (zunächst versteckt)
        let qr_hwnd = CreateWindowExW(
            WS_EX_TOPMOST,
            class_name,
            w!("Windows Remote Steuerung"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            390,
            510,
            None,
            None,
            instance,
            None,
        )?;

        // "URL kopieren" Button
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Verbindungs-URL kopieren"),
            WS_TABSTOP | WS_VISIBLE | WS_CHILD | WINDOW_STYLE(BS_DEFPUSHBUTTON as u32),
            30,
            415,
            160,
            38,
            qr_hwnd,
            HMENU(ID_BTN_COPY as *mut core::ffi::c_void),
            instance,
            None,
        )?;

        // "Schließen" Button
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Schlie\u{00df}en"),
            WS_TABSTOP | WS_VISIBLE | WS_CHILD,
            200,
            415,
            150,
            38,
            qr_hwnd,
            HMENU(ID_BTN_CLOSE as *mut core::ffi::c_void),
            instance,
            None,
        )?;

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
            hWnd: qr_hwnd,
            uID: 1,
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
            uCallbackMessage: WM_TRAYICON,
            hIcon: LoadIconW(None, IDI_APPLICATION)?,
            szTip: tip,
            ..Default::default()
        };

        let _ = Shell_NotifyIconW(NIM_ADD, &nid);

        let _ = APP_CTX.set(TrayAppContext {
            config,
            auth,
            nid,
        });

        let dir = remote_keyboard_lib::config::get_config_dir();
        let _ = std::fs::write(dir.join("ui.log"), format!("Tray & Fenster erstellt. Starte Message Loop...\n"));

        // Message Loop
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = std::fs::write(dir.join("ui.log"), format!("Message Loop beendet mit msg: {}\n", msg.message));

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
                    let _ = SetClipboardData(13, windows::Win32::Foundation::HANDLE(h_mem.0)); // CF_UNICODETEXT = 13
                }
            }
            let _ = CloseClipboard();
        }
    }
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_RBUTTONUP {
                show_tray_menu(hwnd);
            } else if event == WM_LBUTTONUP || event == WM_LBUTTONDBLCLK {
                show_qr_dialog(hwnd);
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xffff) as usize;
            match id {
                ID_TRAY_SHOW_QR => {
                    show_qr_dialog(hwnd);
                }
                ID_TRAY_COPY_URL | ID_BTN_COPY => {
                    let url = get_current_access_url();
                    copy_to_clipboard(&url);
                }
                ID_TRAY_NEW_TOKEN => {
                    if let Some(ctx) = APP_CTX.get() {
                        let new_tok = ctx.config.lock().unwrap().regenerate_token();
                        ctx.auth.set_token(new_tok);
                        let _ = InvalidateRect(hwnd, None, true);
                        show_qr_dialog(hwnd);
                    }
                }
                ID_TRAY_AUTOSTART => {
                    if let Some(ctx) = APP_CTX.get() {
                        let current = is_autostart_registered();
                        let new_state = !current;
                        let _ = set_autostart_registered(new_state);
                        ctx.config.lock().unwrap().autostart = new_state;
                    }
                }
                ID_TRAY_EXIT => {
                    let _ = DestroyWindow(hwnd);
                }
                ID_BTN_CLOSE => {
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            draw_qr_dialog(hwnd, hdc);
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        }
        WM_CLOSE => {
            // Nur verstecken, nicht die Anwendung beenden!
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

        let _ = AppendMenuW(menu, MF_STRING, ID_TRAY_SHOW_QR, w!("QR-Code anzeigen"));
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

unsafe fn show_qr_dialog(hwnd: HWND) {
    let _ = ShowWindow(hwnd, SW_SHOW);
    let _ = SetForegroundWindow(hwnd);
    let _ = InvalidateRect(hwnd, None, true);
}

unsafe fn draw_qr_dialog(hwnd: HWND, hdc: HDC) {
    let mut client_rect = RECT::default();
    let _ = GetClientRect(hwnd, &mut client_rect);

    // OLED Schwarz Hintergrund
    let bg_brush = CreateSolidBrush(COLORREF(0x00000000));
    FillRect(hdc, &client_rect, bg_brush);
    let _ = DeleteObject(bg_brush);

    let access_url = get_current_access_url();

    // QR-Code erzeugen
    if let Ok(code) = QrCode::new(access_url.as_bytes()) {
        let width = code.width();
        let pixel_scale = 6;
        let qr_pixel_size = (width as i32) * pixel_scale;
        let qr_start_x = (client_rect.right - qr_pixel_size) / 2;
        let qr_start_y = 30;

        // Weißer Hintergrund-Rahmen für QR-Code
        let pad = 12;
        let white_brush = CreateSolidBrush(COLORREF(0x00FFFFFF));
        let white_rect = RECT {
            left: qr_start_x - pad,
            top: qr_start_y - pad,
            right: qr_start_x + qr_pixel_size + pad,
            bottom: qr_start_y + qr_pixel_size + pad,
        };
        FillRect(hdc, &white_rect, white_brush);

        // Schwarze QR-Code Module zeichnen
        let black_brush = CreateSolidBrush(COLORREF(0x00000000));
        for y in 0..width {
            for x in 0..width {
                if code[(x, y)] == qrcode::Color::Dark {
                    let rx = qr_start_x + (x as i32) * pixel_scale;
                    let ry = qr_start_y + (y as i32) * pixel_scale;
                    let module_rect = RECT {
                        left: rx,
                        top: ry,
                        right: rx + pixel_scale,
                        bottom: ry + pixel_scale,
                    };
                    FillRect(hdc, &module_rect, black_brush);
                }
            }
        }
        let _ = DeleteObject(black_brush);
        let _ = DeleteObject(white_brush);

        // Text unter dem QR-Code
        SetBkMode(hdc, BACKGROUND_MODE(1)); // TRANSPARENT = 1
        SetTextColor(hdc, COLORREF(0x00F8BDF8)); // Hellblau/Türkis

        let font_title = CreateFontW(
            18, 0, 0, 0, FW_BOLD.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
        );
        let prev_font = SelectObject(hdc, font_title);

        let mut title_rect = RECT {
            left: 20,
            top: qr_start_y + qr_pixel_size + 24,
            right: client_rect.right - 20,
            bottom: qr_start_y + qr_pixel_size + 48,
        };
        let mut title_wide: Vec<u16> = "Mit iPad / iPhone scannen".encode_utf16().collect();
        DrawTextW(hdc, &mut title_wide, &mut title_rect, DT_CENTER | DT_SINGLELINE);

        // Text URL & Info
        let font_info = CreateFontW(
            14, 0, 0, 0, FW_NORMAL.0 as i32, 0, 0, 0, 0, 0, 0, 0, 0, w!("Segoe UI"),
        );
        SelectObject(hdc, font_info);
        SetTextColor(hdc, COLORREF(0x00C0C0C0));

        let mut url_rect = RECT {
            left: 20,
            top: qr_start_y + qr_pixel_size + 52,
            right: client_rect.right - 20,
            bottom: qr_start_y + qr_pixel_size + 110,
        };
        let mut url_wide: Vec<u16> = access_url.encode_utf16().collect();
        DrawTextW(hdc, &mut url_wide, &mut url_rect, DT_CENTER | DT_WORDBREAK | DT_NOPREFIX);

        SelectObject(hdc, prev_font);
        let _ = DeleteObject(font_title);
        let _ = DeleteObject(font_info);
    }
}
