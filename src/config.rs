use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AppConfig {
    pub port: u16,
    pub token: String,
    pub autostart: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            token: generate_random_token(),
            autostart: false,
        }
    }
}

pub fn generate_random_token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}

pub fn get_config_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("remote-keyboard")
}

pub fn get_config_path() -> PathBuf {
    get_config_dir().join("config.json")
}

impl AppConfig {
    pub fn load_or_create() -> Self {
        let path = get_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                    #[cfg(windows)]
                    {
                        config.autostart = is_autostart_registered();
                    }
                    return config;
                }
            }
        }

        let config = AppConfig::default();
        config.save();
        config
    }

    pub fn save(&self) {
        let dir = get_config_dir();
        if let Err(e) = fs::create_dir_all(&dir) {
            error!("Fehler beim Erstellen des Config-Ordners {:?}: {}", dir, e);
            return;
        }

        let path = get_config_path();
        match serde_json::to_string_pretty(self) {
            Ok(json) => {
                if let Err(e) = fs::write(&path, json) {
                    error!("Fehler beim Schreiben der Config {:?}: {}", path, e);
                } else {
                    info!("Konfiguration gespeichert unter {:?}", path);
                }
            }
            Err(e) => error!("Fehler beim Serialisieren der Config: {}", e),
        }
    }

    pub fn regenerate_token(&mut self) -> String {
        self.token = generate_random_token();
        self.save();
        self.token.clone()
    }
}

// Windows Registry Autostart Helper
#[cfg(windows)]
pub fn is_autostart_registered() -> bool {
    use windows::core::w;
    use windows::Win32::System::Registry::*;

    unsafe {
        let mut key = HKEY::default();
        let subkey = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
        if RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_READ, &mut key).is_ok() {
            let value_name = w!("RemoteKeyboardTray");
            let mut data_type = REG_VALUE_TYPE::default();
            let mut data_len = 0u32;
            let status = RegQueryValueExW(key, value_name, None, Some(&mut data_type), None, Some(&mut data_len));
            let _ = RegCloseKey(key);
            status.is_ok()
        } else {
            false
        }
    }
}

#[cfg(windows)]
pub fn set_autostart_registered(enable: bool) -> Result<(), Box<dyn std::error::Error>> {
    use windows::core::w;
    use windows::Win32::System::Registry::*;

    unsafe {
        let mut key = HKEY::default();
        let subkey = w!(r"Software\Microsoft\Windows\CurrentVersion\Run");
        RegOpenKeyExW(HKEY_CURRENT_USER, subkey, 0, KEY_SET_VALUE, &mut key).ok()?;

        let value_name = w!("RemoteKeyboardTray");
        if enable {
            let current_exe = std::env::current_exe()?;
            let exe_str = current_exe.to_str().ok_or("Ungültiger Pfad")?;
            let mut wide_chars: Vec<u16> = exe_str.encode_utf16().collect();
            wide_chars.push(0);

            let slice = std::slice::from_raw_parts(
                wide_chars.as_ptr() as *const u8,
                wide_chars.len() * 2,
            );

            RegSetValueExW(
                key,
                value_name,
                0,
                REG_SZ,
                Some(slice),
            ).ok()?;
            info!("Windows-Autostart erfolgreich aktiviert");
        } else {
            let _ = RegDeleteValueW(key, value_name);
            info!("Windows-Autostart erfolgreich deaktiviert");
        }

        let _ = RegCloseKey(key);
    }
    Ok(())
}
