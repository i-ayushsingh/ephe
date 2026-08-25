use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
        Query,
    },
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Router,
};
use futures::{SinkExt, StreamExt};
use qrcode::render::svg;
use qrcode::QrCode;
use serde::Deserialize;
use tokio::sync::mpsc;

use crate::manifest;
use crate::session::{
    is_valid_token, random_id, random_token, Client, ClientMsg, DeviceInfo, ServerMsg,
    SharedSession, MAX_MOBILES,
};

struct AppState {
    session: SharedSession,
    port: u16,
    lan_ip: String,
    /// WS heartbeat period in seconds; peers silent for 3x this get dropped.
    heartbeat_secs: u64,
}

pub fn build_router(session: SharedSession, port: u16, lan_ip: String) -> Router {
    // ponytail: env knob so a test can shrink the heartbeat; default 10s.
    let heartbeat_secs: u64 = std::env::var("EPHE_HEARTBEAT_SECS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(10);
    let state = Arc::new(AppState {
        session,
        port,
        lan_ip,
        heartbeat_secs,
    });
    Router::new()
        .route("/", get(index))
        .route("/style.css", get(css))
        .route("/app.js", get(js))
        .route("/logo.png", get(logo))
        .route("/text-logo.png", get(text_logo))
        .route("/font.ttf", get(pixel_font))
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
    (
        [("content-type", "text/css; charset=utf-8")],
        include_str!("style.css"),
    )
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
    let url = format!("http://{}:{}/?token={}", state.lan_ip, state.port, token);
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
    (
        [("content-type", "image/png")],
        &include_bytes!("../text-logo.png")[..],
    )
        .into_response()
}

/// The bundled pixel font (Press Start 2P, OFL) for the "pixel" editor font.
async fn pixel_font() -> Response {
    (
        [("content-type", "font/ttf")],
        &include_bytes!("../PressStart2P-Regular.ttf")[..],
    )
        .into_response()
}

async fn logo() -> Response {
    (
        [("content-type", "image/png")],
        &include_bytes!("../Logo.png")[..],
    )
        .into_response()
}

const MAX_TEXT_LEN: usize = 1_048_576; // 1 MB payload limit

async fn reset(
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    // ponytail: require X-Ephe-Client custom header to protect loopback/LAN
    // endpoints from cross-origin CSRF (e.g. malicious webpages executing
    // no-cors fetch/form submissions to localhost).
    if !headers.contains_key("x-ephe-client") {
        return StatusCode::FORBIDDEN.into_response();
    }

    let token = q.token.unwrap_or_default();
    let mut s = state.session.lock().await;
    // ponytail: loopback = the user's own machine, trusted exactly like the
    // desktop WS join (tokenless). LAN clients still need the token — the
    // host-only reset privilege never escapes to the network.
    if !addr.ip().is_loopback() && !is_valid_token(&s, &token) {
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
    s.locked = false;
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
    ws.max_frame_size(MAX_TEXT_LEN + 4096)
        .max_message_size(MAX_TEXT_LEN + 4096)
        .on_upgrade(move |socket| handle_socket(socket, state, is_loopback, token))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, is_loopback: bool, token: String) {
    let role: &'static str = if is_loopback { "desktop" } else { "mobile" };
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMsg>();
    let (mut sink, mut stream) = socket.split();

    // accept into session
    let (accepted, locked) = {
        let mut s = state.session.lock().await;
        match accept_into_session(&mut s, role, &token, tx.clone()) {
            Ok(()) => (s.text.clone(), s.locked),
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

    let _ = tx.send(ServerMsg::Init {
        text: accepted,
        locked,
    });
    publish_manifest(&state).await;
    // ponytail: include the joiner so a second phone immediately shows as
    // connected (and gets the join toast) instead of staying "waiting".
    broadcast(&state, ServerMsg::PeerJoined, role, None).await;
    // the desktop needs the fresh device list (ids + labels) right after a join
    {
        let s = state.session.lock().await;
        if let Some(d) = s.desktop.as_ref() {
            let _ = d.tx.send(ServerMsg::Devices {
                devices: device_list(&s),
            });
        }
    }

    // Heartbeat: ping every `heartbeat_secs`; browsers auto-pong at the
    // protocol level (even in background tabs). A peer that stops answering
    // (app force-killed, Wi-Fi dropped, airplane mode) gets no pong, so its
    // liveness deadline passes and we drop it — otherwise the session would
    // linger with a phantom peer forever.
    let period = Duration::from_secs(state.heartbeat_secs);
    let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // absolute deadline, refreshed only when a frame actually arrives
    let mut deadline = tokio::time::Instant::now() + period * 3;

    let mut should_close = false;
    loop {
        tokio::select! {
            biased;
            _ = heartbeat.tick() => {
                if sink.send(Message::Ping(vec![])).await.is_err() {
                    break;
                }
            }
            msg = rx.recv() => {
                let Some(msg) = msg else { break; };
                let payload = match serde_json::to_string(&msg) {
                    Ok(s) => s,
                    Err(_) => break,
                };
                if sink.send(Message::Text(payload)).await.is_err() {
                    break;
                }
                if matches!(msg, ServerMsg::PeerLeft | ServerMsg::Reset { .. } | ServerMsg::Kicked) {
                    should_close = true;
                    break;
                }
            }
            ws_msg = stream.next() => {
                match ws_msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Ok(cmsg) = serde_json::from_str::<ClientMsg>(&t) {
                            match cmsg {
                                ClientMsg::Text { text } => {
                                    if text.len() > MAX_TEXT_LEN {
                                        continue;
                                    }
                                    {
                                        let mut s = state.session.lock().await;
                                        // ponytail: enforce lock server-side — frontend lock is
                                        // cosmetic, the backend is the authority. Only the desktop
                                        // can write when the pad is locked.
                                        if s.locked && role != "desktop" {
                                            continue;
                                        }
                                        s.text = text.clone();
                                    }
                                    broadcast(&state, ServerMsg::Text { text }, role, Some(&tx))
                                        .await;
                                }
                                ClientMsg::Typing => {
                                    broadcast(&state, ServerMsg::Typing, role, Some(&tx)).await;
                                }
                                ClientMsg::Lock { locked } => {
                                    // ponytail: single-writer is desktop-only; phones can't lock
                                    if role == "desktop" {
                                        {
                                            let mut s = state.session.lock().await;
                                            s.locked = locked;
                                        }
                                        broadcast(&state, ServerMsg::Locked { locked }, role, Some(&tx))
                                            .await;
                                    }
                                }
                                ClientMsg::Ack => {
                                    // a phone received someone's text; relay the
                                    // read receipt to everyone but the acker
                                    if role == "mobile" {
                                        let label = {
                                            let s = state.session.lock().await;
                                            s.mobiles
                                                .iter()
                                                .position(|m| m.tx.same_channel(&tx))
                                                .map(|i| format!("phone {}", i + 1))
                                        };
                                        if let Some(label) = label {
                                            broadcast(&state, ServerMsg::Seen { label }, role, Some(&tx))
                                                .await;
                                        }
                                    }
                                }
                                ClientMsg::Kick { id } => {
                                    // desktop-only: remove the phone, it gets
                                    // "kicked" and closes; its disconnect wipes
                                    // the session and peer_lefts everyone (ephemeral)
                                    if role == "desktop" {
                                        let mut s = state.session.lock().await;
                                        if let Some(pos) = s.mobiles.iter().position(|m| m.id == id) {
                                            let kicked = s.mobiles.remove(pos);
                                            let _ = kicked.tx.send(ServerMsg::Kicked);
                                        }
                                    }
                                }
                            }
                        }
                        deadline = tokio::time::Instant::now() + period * 3;
                    }
                    Some(Ok(_)) => {
                        // pongs (and anything else) prove the peer is alive
                        deadline = tokio::time::Instant::now() + period * 3;
                    }
                    Some(Err(_)) | None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => {
                println!("ephe: heartbeat timeout, dropping silent {role}");
                break;
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
        s.desktop = Some(Client {
            id: random_id(),
            tx,
        });
    } else {
        if !is_valid_token(s, token) {
            return Err("invalid token");
        }
        if s.mobiles.len() >= MAX_MOBILES {
            return Err("session full: max 2 mobiles");
        }
        s.mobiles.push(Client {
            id: random_id(),
            tx,
        });
    }
    Ok(())
}

fn device_list(s: &crate::session::Session) -> Vec<DeviceInfo> {
    s.mobiles
        .iter()
        .enumerate()
        .map(|(i, m)| DeviceInfo {
            id: m.id.clone(),
            label: format!("phone {}", i + 1),
        })
        .collect()
}

async fn on_disconnect(state: &AppState, role: &str, me: &mpsc::UnboundedSender<ServerMsg>) {
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
    publish_manifest(state).await;
}

/// Refresh the PortPeek manifest after any join/leave (device count changed).
async fn publish_manifest(state: &AppState) {
    let (id, connected) = {
        let s = state.session.lock().await;
        (s.id.clone(), s.desktop.is_some() as usize + s.mobiles.len())
    };
    manifest::publish(&id, state.port, connected);
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
