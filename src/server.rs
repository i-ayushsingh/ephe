use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::{SinkExt, StreamExt};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::session::{
    is_valid_token, random_token, Client, ClientMsg, MAX_MOBILES, ServerMsg, SharedSession,
};

struct AppState {
    session: SharedSession,
    port: u16,
    lan_ip: String,
}

pub fn build_router(
    session: SharedSession,
    port: u16,
    lan_ip: String,
) -> Router {
    let state = Arc::new(AppState { session, port, lan_ip });
    Router::new()
        .route("/", get(index))
        .route("/style.css", get(css))
        .route("/app.js", get(js))
        .route("/logo.png", get(logo))
        .route("/text-logo.png", get(text_logo))
        .route("/qr", get(qr))
        .route("/url", get(mobile_url))
        .route("/reset", post(reset))
        .route("/ws", get(ws))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("index.html"))
}

async fn css() -> Response {
    ([("content-type", "text/css; charset=utf-8")], include_str!("style.css"))
        .into_response()
}

async fn js() -> Response {
    (
        [("content-type", "application/javascript; charset=utf-8")],
        include_str!("app.js"),
    )
        .into_response()
}

async fn qr(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Response {
    let token = state.session.lock().await.token.clone();
    let url = format!(
        "http://{}:{}/?token={}",
        state.lan_ip, state.port, token
    );
    let code = match QrCode::new(url.as_bytes()) {
        Ok(c) => c,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let svg = code
        .render::<svg::Color<'_>>()
        .min_dimensions(180, 180)
        .build();
    ([("content-type", "image/svg+xml")], svg).into_response()
}

/// The mobile URL as plain text, so the desktop can show it as a QR fallback.
async fn mobile_url(axum::extract::State(state): axum::extract::State<Arc<AppState>>) -> Response {
    let token = state.session.lock().await.token.clone();
    let url = format!("http://{}:{}/?token={}", state.lan_ip, state.port, token);
    let body = serde_json::json!({ "url": url }).to_string();
    ([("content-type", "application/json")], body).into_response()
}

async fn text_logo() -> Response {
    let path = find_named("text-logo.png");
    match path {
        Some(p) => match std::fs::read(&p) {
            Ok(bytes) => ([("content-type", "image/png")], bytes).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn logo() -> Response {
    let path = find_named("Logo.png");
    match path {
        Some(p) => match std::fs::read(&p) {
            Ok(bytes) => ([("content-type", "image/png")], bytes).into_response(),
            Err(_) => StatusCode::NOT_FOUND.into_response(),
        },
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

fn find_named(name: &str) -> Option<PathBuf> {
    let candidates = [
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())),
        std::env::current_dir().ok(),
    ];
    for base in candidates.into_iter().flatten() {
        let p = base.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

async fn reset(
    Query(q): Query<WsQuery>,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let token = q.token.unwrap_or_default();
    let mut s = state.session.lock().await;
    if !is_valid_token(&s, &token) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let new_token = random_token();
    let new_desktop_url = format!(
        "http://localhost:{}/?role=desktop&token={}",
        state.port, new_token
    );
    let new_mobile_url = format!(
        "http://{}:{}/?token={}",
        state.lan_ip, state.port, new_token
    );

    // ponytail: tell the desktop to navigate, the mobiles to leave, then
    // rotate the token and wipe the slate.
    if let Some(c) = s.desktop.as_ref() {
        let _ = c.tx.send(ServerMsg::Reset {
            url: new_desktop_url.clone(),
        });
    }
    for m in s.mobiles.iter() {
        let _ = m.tx.send(ServerMsg::PeerLeft);
    }
    s.desktop = None;
    s.mobiles.clear();
    s.text.clear();
    s.token = new_token;

    println!("ephe: session reset");
    println!("ephe: new desktop: {}", new_desktop_url);
    println!("ephe: new mobile : {}", new_mobile_url);

    let body = serde_json::json!({ "url": new_desktop_url }).to_string();
    ([("content-type", "application/json")], body).into_response()
}

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn ws(
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let is_loopback = addr.ip().is_loopback();
    let token = q.token.unwrap_or_default();
    ws.on_upgrade(move |socket| handle_socket(socket, state, is_loopback, token))
}

async fn handle_socket(
    socket: WebSocket,
    state: Arc<AppState>,
    is_loopback: bool,
    token: String,
) {
    let role: &'static str = if is_loopback { "desktop" } else { "mobile" };
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let (mut sink, mut stream) = socket.split();

    // accept into session
    let accepted = {
        let mut s = state.session.lock().await;
        match accept_into_session(&mut s, role, &token, tx.clone()) {
            Ok(()) => s.text.clone(),
            Err(reason) => {
                let _ = sink
                    .send(Message::Text(
                        serde_json::json!({"type": "error", "reason": reason}).to_string(),
                    ))
                    .await;
                let _ = sink.close().await;
                return;
            }
        }
    };

    let _ = tx.send(ServerMsg::Init { text: accepted });
    // ponytail: include the joiner so a second phone immediately shows as
    // connected (and gets the join toast) instead of staying "waiting".
    broadcast(&state, ServerMsg::PeerJoined, role, None).await;

    let mut should_close = false;
    loop {
        tokio::select! {
            biased;
            msg = rx.recv() => {
                let Some(msg) = msg else { break; };
                let payload = match serde_json::to_string(&msg) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                if sink.send(Message::Text(payload)).await.is_err() {
                    break;
                }
                if matches!(msg, ServerMsg::PeerLeft | ServerMsg::Reset { .. }) {
                    should_close = true;
                    break;
                }
            }
            ws_msg = stream.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(ClientMsg::Text { text }) = serde_json::from_str(&t) {
                            {
                                let mut s = state.session.lock().await;
                                s.text = text.clone();
                            }
                            broadcast(&state, ServerMsg::Text { text }, role, Some(&tx)).await;
                        }
                    }
                    Some(Ok(_)) => continue,
                    Some(Err(_)) | None => break,
                }
            }
        }
    }

    if should_close {
        let _ = sink.close().await;
    }
    on_disconnect(&state, role, &tx).await;
}

fn accept_into_session(
    s: &mut crate::session::Session,
    role: &str,
    token: &str,
    tx: mpsc::UnboundedSender<ServerMsg>,
) -> Result<(), &'static str> {
    if role == "desktop" {
        if s.desktop.is_some() {
            return Err("desktop already connected");
        }
        s.desktop = Some(Client { tx });
    } else {
        if !is_valid_token(s, token) {
            return Err("invalid token");
        }
        if s.mobiles.len() >= MAX_MOBILES {
            return Err("session full: max 2 mobiles");
        }
        s.mobiles.push(Client { tx });
    }
    Ok(())
}

async fn on_disconnect(
    state: &AppState,
    role: &str,
    me: &mpsc::UnboundedSender<ServerMsg>,
) {
    let others = {
        let mut s = state.session.lock().await;
        if role == "desktop" {
            s.desktop = None;
        } else {
            s.mobiles.retain(|m| !m.tx.same_channel(me));
        }
        // ponytail: ephemeral by design — wipe on any disconnect
        s.text.clear();
        let mut txs = Vec::new();
        if role == "desktop" {
            txs.extend(s.mobiles.iter().map(|m| m.tx.clone()));
        } else {
            if let Some(d) = s.desktop.as_ref() {
                txs.push(d.tx.clone());
            }
            txs.extend(s.mobiles.iter().map(|m| m.tx.clone()));
        }
        txs
    };
    for t in others {
        let _ = t.send(ServerMsg::PeerLeft);
    }
}

/// Send `msg` to every peer except `exclude` (the sender).
async fn broadcast(
    state: &AppState,
    msg: ServerMsg,
    from_role: &str,
    exclude: Option<&mpsc::UnboundedSender<ServerMsg>>,
) {
    let targets = {
        let s = state.session.lock().await;
        let mut txs: Vec<_> = if from_role == "desktop" {
            s.mobiles.iter().map(|m| m.tx.clone()).collect()
        } else {
            s.desktop
                .as_ref()
                .map(|d| d.tx.clone())
                .into_iter()
                .chain(s.mobiles.iter().map(|m| m.tx.clone()))
                .collect()
        };
        if let Some(ex) = exclude {
            txs.retain(|t| !t.same_channel(ex));
        }
        txs
    };
    for t in targets {
        let _ = t.send(msg.clone());
    }
}