use local_ip_address::local_ip;
use qrcode::{render::unicode::Dense1x2, QrCode};
use remote_keyboard_lib::{
    auth::AuthState,
    config::AppConfig,
    server::{create_router, AppState},
};
use std::net::SocketAddr;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Tracing Logger initialisieren
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Port und Konfiguration laden
    let config = AppConfig::load_or_create();
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(config.port);

    // Lokale IP-Adresse im Netzwerk ermitteln
    let ip = local_ip().unwrap_or_else(|_| "127.0.0.1".parse().unwrap());

    // AuthState mit dem Token initialisieren
    let auth = AuthState::with_token(config.token.clone());
    let token = auth.get_token();

    // Vollständige Verbindungs-URL für iPad / iPhone
    let access_url = format!("http://{}:{}/?token={}", ip, port, token);

    // QR-Code im Terminal erzeugen
    let code = QrCode::new(access_url.as_bytes())?;
    let image = code
        .render::<Dense1x2>()
        .dark_color(Dense1x2::Dark)
        .light_color(Dense1x2::Light)
        .build();

    // Konsolen-Banner ausgeben
    println!("\n=======================================================");
    println!("     WINDOWS REMOTE-STEUERUNG (RUST SERVER)");
    println!("=======================================================\n");
    println!("Scanne diesen QR-Code mit der Kamera deines iPads/iPhones,\num die Fernsteuerung direkt im Browser zu öffnen:\n");
    println!("{}", image);
    println!("\n-------------------------------------------------------");
    println!("Manuelle Verbindungs-URL:");
    println!("  {}", access_url);
    println!("\nSicherheitstoken (Token):");
    println!("  {}", token);
    println!("-------------------------------------------------------\n");
    println!("Tipp: Im Safari-Menü auf 'Zum Home-Bildschirm' tippen für");
    println!("ein nahtloses Vollbild-Trackpad ohne Browserleisten.\n");
    println!("Server läuft und lauscht auf allen Schnittstellen (0.0.0.0:{})...", port);
    println!("Beenden jederzeit mit Strg + C.\n");

    let app_state = AppState { auth };
    let app = create_router(app_state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!("HTTP & WebSocket Server gestartet auf http://0.0.0.0:{}", port);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    println!("\nServer wurde sauber beendet. Auf Wiedersehen!");
    Ok(())
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Fehler beim Warten auf Beenden-Signal (Ctrl+C)");
    println!("\nBeenden-Signal empfangen...");
}
