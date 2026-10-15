use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query, State,
    },
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use futures_util::{SinkExt, StreamExt};
use rust_embed::RustEmbed;
use serde::Deserialize;
use tracing::{error, info, warn};

use crate::{
    auth::AuthState,
    input::{handle_client_message, ClientMessage},
};

#[derive(RustEmbed)]
#[folder = "static/"]
struct Assets;

#[derive(Clone)]
pub struct AppState {
    pub auth: AuthState,
}

#[derive(Deserialize)]
pub struct WsQuery {
    pub token: Option<String>,
}

pub fn create_router(app_state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_handler))
        .fallback(static_handler)
        .with_state(app_state)
}

// Handler für statische Dateien aus rust-embed
async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let mut path = uri.path().trim_start_matches('/').to_string();
    if path.is_empty() {
        path = "index.html".to_string();
    }

    match Assets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, HeaderValue::from_str(mime.as_ref()).unwrap())],
                content.data,
            )
                .into_response()
        }
        None => {
            // SPA Fallback zu index.html
            match Assets::get("index.html") {
                Some(index_content) => (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
                    index_content.data,
                )
                    .into_response(),
                None => (StatusCode::NOT_FOUND, "404 Not Found").into_response(),
            }
        }
    }
}

// WebSocket Upgrade Handler mit Token-Prüfung
async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsQuery>,
    State(state): State<AppState>,
) -> Response {
    let token = params.token.unwrap_or_default();
    if !state.auth.is_valid(&token) {
        warn!("Verbindungsversuch mit ungültigem Token abgelehnt: {}", token);
        return (StatusCode::UNAUTHORIZED, "Ungültiger Authentifizierungstoken").into_response();
    }

    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

// WebSocket Session Loop
async fn handle_socket(socket: WebSocket, _state: AppState) {
    let (mut sender, mut receiver) = socket.split();
    info!("Neuer Client verbunden via WebSocket");

    while let Some(result) = receiver.next().await {
        match result {
            Ok(msg) => match msg {
                Message::Text(text) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(client_msg) => {
                        if let ClientMessage::Ping = client_msg {
                            let pong = serde_json::json!({ "type": "Pong" }).to_string();
                            if let Err(e) = sender.send(Message::Text(pong)).await {
                                error!("Fehler beim Senden von Pong: {}", e);
                                break;
                            }
                        } else {
                            handle_client_message(client_msg);
                        }
                    }
                    Err(err) => {
                        warn!("Ungültiges JSON-Paket empfangen: {} (Fehler: {})", text, err);
                    }
                },
                Message::Binary(bin) => {
                    warn!("Binärnachricht empfangen (wird ignoriert): {} Bytes", bin.len());
                }
                Message::Ping(payload) => {
                    let _ = sender.send(Message::Pong(payload)).await;
                }
                Message::Pong(_) => {}
                Message::Close(_) => {
                    info!("Client hat Verbindung geschlossen");
                    break;
                }
            },
            Err(e) => {
                warn!("WebSocket-Übertragungsfehler: {}", e);
                break;
            }
        }
    }

    info!("Client getrennt");
}
