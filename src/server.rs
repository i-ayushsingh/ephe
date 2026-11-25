use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::{
        connect_info::ConnectInfo,
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Path, Query,
    },
    http::{header, HeaderMap, StatusCode},
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
    is_valid_token, random_id, random_token, Client, ClientMsg, DeviceInfo, EphemeralFile,
    EphemeralFileMeta, ServerMsg, SharedSession, MAX_FILE_SIZE, MAX_MOBILES,
};

struct AppState {
    session: SharedSession,
    port: u16,
    lan_ip: String,
    heartbeat_secs: u64,
}

pub fn build_router(session: SharedSession, port: u16, lan_ip: String) -> Router {
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
        .route(
            "/upload",
            post(upload_file).layer(DefaultBodyLimit::max(MAX_FILE_SIZE + 4096)),
        )
        .route("/file/:id", get(download_file))
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

#[derive(Deserialize)]
struct QrQuery {
    ip: Option<String>,
}

/// QR code points to the LAN URL with the session token embedded for 1-scan pairing.
async fn qr(
    Query(q): Query<QrQuery>,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let ip =
        q.ip.filter(|candidate| crate::net::list_ips().iter().any(|c| &c.ip == candidate))
            .unwrap_or_else(|| state.lan_ip.clone());
    let token = state.session.lock().await.token.clone();
    let url = format!("http://{}:{}/?token={}", ip, state.port, token);
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

/// The mobile URL (with token) and available LAN interfaces.
async fn mobile_url(
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let s = state.session.lock().await;
    let token = q.token.unwrap_or_default();
    // Allow loopback (desktop host UI) or already-authenticated clients
    if !addr.ip().is_loopback() && !is_valid_token(&s, &token) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let url = format!("http://{}:{}/?token={}", state.lan_ip, state.port, s.token);
    let candidates: Vec<_> = crate::net::list_ips()
        .into_iter()
        .map(|c| serde_json::json!({ "name": c.name, "ip": c.ip }))
        .collect();
    let body = serde_json::json!({
        "url": url,
        "port": state.port,
        "ip": state.lan_ip,
        "candidates": candidates
    })
    .to_string();
    ([("content-type", "application/json")], body).into_response()
}

const BLOCKED_EXTENSIONS: &[&str] = &[
    ".exe", ".bat", ".cmd", ".ps1", ".scr", ".vbs", ".msi", ".dll", ".com", ".pif", ".hta", ".cpl",
    ".jar",
];

fn percent_decode(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = (bytes[i + 1] as char).to_digit(16);
            let h2 = (bytes[i + 2] as char).to_digit(16);
            if let (Some(a), Some(b)) = (h1, h2) {
                out.push(((a << 4) | b) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn sanitize_filename(raw: &str) -> String {
    let decoded = percent_decode(raw);
    let base = decoded.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = base
        .chars()
        .filter(|&c| c != '"' && !c.is_control() && c != '/' && c != '\\')
        .collect();
    let trimmed = clean.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." {
        "file".to_string()
    } else {
        trimmed.to_string()
    }
}

fn is_blocked_extension(filename: &str) -> bool {
    let lower = filename.trim_end_matches('.').to_ascii_lowercase();
    BLOCKED_EXTENSIONS.iter().any(|ext| lower.ends_with(ext))
}

/// Upload an ephemeral in-memory file (max 25MB). Never saved to disk.
async fn upload_file(
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
    body: axum::body::Bytes,
) -> Response {
    let token = q.token.unwrap_or_default();
    {
        let s_check = state.session.lock().await;
        if !addr.ip().is_loopback() && !is_valid_token(&s_check, &token) {
            return StatusCode::FORBIDDEN.into_response();
        }
    }

    if body.len() > MAX_FILE_SIZE {
        return (StatusCode::PAYLOAD_TOO_LARGE, "file exceeds 25 MB limit").into_response();
    }

    let filename = headers
        .get("x-filename")
        .and_then(|v| v.to_str().ok())
        .map(sanitize_filename)
        .unwrap_or_else(|| "file".to_string());

    if is_blocked_extension(&filename) {
        return (
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "executable/script file types are blocked",
        )
            .into_response();
    }

    let mime = headers
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let file_id = random_id();
    let meta = EphemeralFileMeta {
        id: file_id.clone(),
        name: filename,
        size: body.len(),
        mime,
    };

    let eph_file = EphemeralFile {
        meta: meta.clone(),
        data: body.to_vec(),
    };

    {
        let mut s = state.session.lock().await;
        let current_total: usize = s.files.iter().map(|f| f.data.len()).sum();
        if current_total.saturating_add(body.len()) > MAX_FILE_SIZE {
            return (
                StatusCode::PAYLOAD_TOO_LARGE,
                "session file storage exceeds 25 MB limit",
            )
                .into_response();
        }
        s.files.push(eph_file);
    }

    broadcast(
        &state,
        ServerMsg::FileAvailable { file: meta },
        "server",
        None,
    )
    .await;
    (
        StatusCode::OK,
        serde_json::json!({ "id": file_id }).to_string(),
    )
        .into_response()
}

/// Download an ephemeral in-memory file.
async fn download_file(
    Path(id): Path<String>,
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    let token = q.token.unwrap_or_default();
    let s = state.session.lock().await;
    if !addr.ip().is_loopback() && !is_valid_token(&s, &token) {
        return StatusCode::FORBIDDEN.into_response();
    }
    if let Some(f) = s.files.iter().find(|f| f.meta.id == id) {
        let mut resp = f.data.clone().into_response();
        if let Ok(mime_val) = f.meta.mime.parse() {
            resp.headers_mut().insert(header::CONTENT_TYPE, mime_val);
        }
        let disposition = format!("attachment; filename=\"{}\"", f.meta.name);
        if let Ok(disp_val) = disposition.parse() {
            resp.headers_mut()
                .insert(header::CONTENT_DISPOSITION, disp_val);
        }
        resp
    } else {
        StatusCode::NOT_FOUND.into_response()
    }
}

async fn text_logo() -> Response {
    (
        [("content-type", "image/png")],
        &include_bytes!("../text-logo.png")[..],
    )
        .into_response()
}

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
    if !headers.contains_key("x-ephe-client") {
        return StatusCode::FORBIDDEN.into_response();
    }

    let token = q.token.unwrap_or_default();
    let mut s = state.session.lock().await;
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

    if let Some(c) = s.desktop.as_ref() {
        let _ = c.tx.try_send(ServerMsg::Reset {
            url: new_desktop_url.clone(),
        });
    }
    for m in s.mobiles.iter() {
        let _ = m.tx.try_send(ServerMsg::Reset {
            url: new_mobile_url.clone(),
        });
    }
    s.desktop = None;
    s.mobiles.clear();
    s.files.clear();
    s.text.clear();
    s.locked = false;
    s.token = new_token;
    s.disconnect_gen = s.disconnect_gen.wrapping_add(1);

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

fn is_valid_ws_origin(origin_hdr: Option<&str>, lan_ip: &str, port: u16) -> bool {
    let Some(origin) = origin_hdr else {
        return true;
    };
    let lower = origin.trim().to_ascii_lowercase();
    let (scheme, rest) = if let Some(r) = lower.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = lower.strip_prefix("https://") {
        ("https", r)
    } else {
        return false;
    };

    if rest.is_empty()
        || rest.contains('/')
        || rest.contains('?')
        || rest.contains('#')
        || rest.contains('@')
    {
        return false;
    }

    let (host, origin_port) = if rest.starts_with('[') {
        let Some(end_bracket) = rest.find(']') else {
            return false;
        };
        let host = &rest[..=end_bracket];
        let after = &rest[end_bracket + 1..];
        if after.is_empty() {
            (host, None)
        } else if let Some(p_str) = after.strip_prefix(':') {
            let Ok(p) = p_str.parse::<u16>() else {
                return false;
            };
            (host, Some(p))
        } else {
            return false;
        }
    } else if let Some((h, p_str)) = rest.split_once(':') {
        let Ok(p) = p_str.parse::<u16>() else {
            return false;
        };
        (h, Some(p))
    } else {
        (rest, None)
    };

    let effective_port = origin_port.unwrap_or(if scheme == "https" { 443 } else { 80 });
    if effective_port != port {
        return false;
    }

    let lan_lower = lan_ip.to_ascii_lowercase();
    let is_private_v4 = host
        .parse::<std::net::Ipv4Addr>()
        .is_ok_and(|ip| ip.is_private());
    host == "localhost"
        || host == "127.0.0.1"
        || host == "[::1]"
        || host == lan_lower
        || host == format!("[{lan_lower}]")
        || is_private_v4
}

async fn ws(
    Query(q): Query<WsQuery>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<Arc<AppState>>,
) -> Response {
    // Validate Origin header to protect against Cross-Site WebSocket Hijacking (CSWSH)
    let origin_val = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    if !is_valid_ws_origin(origin_val, &state.lan_ip, state.port) {
        eprintln!(
            "ephe: rejected ws connection from untrusted origin: {:?}",
            origin_val
        );
        return StatusCode::FORBIDDEN.into_response();
    }

    let is_loopback = addr.ip().is_loopback();
    let token = q.token.unwrap_or_default();
    ws.max_frame_size(MAX_TEXT_LEN + 4096)
        .max_message_size(MAX_TEXT_LEN + 4096)
        .on_upgrade(move |socket| handle_socket(socket, state, is_loopback, token))
}

async fn handle_socket(socket: WebSocket, state: Arc<AppState>, is_loopback: bool, token: String) {
    let role: &'static str = if is_loopback { "desktop" } else { "mobile" };
    let (tx, mut rx) = mpsc::channel::<ServerMsg>(128);
    let (mut sink, mut stream) = socket.split();

    // accept into session
    let (accepted, locked, files) = {
        let mut s = state.session.lock().await;
        match accept_into_session(&mut s, role, &token, tx.clone()) {
            Ok(()) => (
                s.text.clone(),
                s.locked,
                s.files.iter().map(|f| f.meta.clone()).collect::<Vec<_>>(),
            ),
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

    let _ = tx.try_send(ServerMsg::Init {
        text: accepted,
        locked,
        files,
    });
    publish_manifest(&state).await;
    broadcast(&state, ServerMsg::PeerJoined, role, None).await;

    // desktop needs the fresh device list right after a join
    {
        let s = state.session.lock().await;
        if let Some(d) = s.desktop.as_ref() {
            let _ = d.tx.try_send(ServerMsg::Devices {
                devices: device_list(&s),
            });
        }
    }

    let period = Duration::from_secs(state.heartbeat_secs);
    let mut heartbeat = tokio::time::interval_at(tokio::time::Instant::now() + period, period);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
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
                                    if role == "desktop" {
                                        let mut s = state.session.lock().await;
                                        if let Some(pos) = s.mobiles.iter().position(|m| m.id == id) {
                                            let kicked = s.mobiles.remove(pos);
                                            let _ = kicked.tx.try_send(ServerMsg::Kicked);
                                            let mut others = Vec::new();
                                            if let Some(d) = s.desktop.as_ref() {
                                                others.push(d.tx.clone());
                                            }
                                            others.extend(s.mobiles.iter().map(|m| m.tx.clone()));
                                            for t in others {
                                                let _ = t.try_send(ServerMsg::PeerLeft);
                                            }
                                        }
                                    }
                                }
                                ClientMsg::DeleteFile { id } => {
                                    {
                                        let mut s = state.session.lock().await;
                                        s.files.retain(|f| f.meta.id != id);
                                    }
                                    broadcast(&state, ServerMsg::FileRemoved { id }, "server", None).await;
                                }
                                ClientMsg::Ping { ts } => {
                                    let _ = tx.try_send(ServerMsg::Pong { ts });
                                }
                            }
                        }
                        deadline = tokio::time::Instant::now() + period * 3;
                    }
                    Some(Ok(_)) => {
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
    on_disconnect(state, role, &tx).await;
}

fn accept_into_session(
    s: &mut crate::session::Session,
    role: &str,
    token: &str,
    tx: mpsc::Sender<ServerMsg>,
) -> Result<(), &'static str> {
    if role == "desktop" {
        // Desktop can reconnect cleanly on reload or tab recovery
        s.desktop = Some(Client {
            id: random_id(),
            tx,
        });
        s.disconnect_gen = s.disconnect_gen.wrapping_add(1);
    } else {
        if !is_valid_token(s, token) {
            return Err("invalid token");
        }
        s.mobiles.retain(|m| !m.tx.is_closed());
        if s.mobiles.len() >= MAX_MOBILES {
            // Evict oldest closed or stale mobile connection if present
            return Err("session full: max 2 mobiles");
        }
        s.disconnect_gen = s.disconnect_gen.wrapping_add(1);
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

fn grace_period_secs(heartbeat_secs: u64, default_secs: u64) -> u64 {
    if heartbeat_secs <= 1 {
        2
    } else {
        std::env::var("EPHE_GRACE_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default_secs)
    }
}

async fn on_disconnect(state: Arc<AppState>, role: &str, me: &mpsc::Sender<ServerMsg>) {
    if role == "desktop" {
        let gen = {
            let mut s = state.session.lock().await;
            s.desktop = None;
            s.disconnect_gen = s.disconnect_gen.wrapping_add(1);
            s.disconnect_gen
        };

        // Desktop reload grace period: don't wipe data immediately!
        // Allow desktop 180 seconds (3 minutes) by default to reconnect on page reload or tab switch.
        let grace_secs = grace_period_secs(state.heartbeat_secs, 180);
        let state_clone = Arc::clone(&state);
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(grace_secs)).await;
            let others = {
                let mut s = state_clone.session.lock().await;
                if s.disconnect_gen == gen && s.desktop.is_none() {
                    s.text.clear();
                    s.files.clear();
                    let mut txs = Vec::new();
                    txs.extend(s.mobiles.iter().map(|m| m.tx.clone()));
                    txs
                } else {
                    Vec::new()
                }
            };
            for t in others {
                let _ = t.try_send(ServerMsg::PeerLeft);
            }
            publish_manifest(&state_clone).await;
        });
        return;
    }

    // role == "mobile"
    let (desktop_alive, remaining_mobiles, gen) = {
        let mut s = state.session.lock().await;
        s.mobiles.retain(|m| !m.tx.same_channel(me));
        s.disconnect_gen = s.disconnect_gen.wrapping_add(1);
        (s.desktop.is_some(), s.mobiles.len(), s.disconnect_gen)
    };

    publish_manifest(&state).await;

    if !desktop_alive {
        let mut s = state.session.lock().await;
        if s.mobiles.is_empty() {
            s.text.clear();
            s.files.clear();
        }
        return;
    }

    // Desktop is alive! Update desktop's device list
    {
        let s = state.session.lock().await;
        if let Some(d) = s.desktop.as_ref() {
            let _ = d.tx.try_send(ServerMsg::Devices {
                devices: device_list(&s),
            });
        }
    }

    if remaining_mobiles > 0 {
        return;
    }

    // All phones disconnected, desktop still active.
    // Give mobile a grace period (e.g. 60s) to reconnect without clearing desktop's text.
    let grace_secs = grace_period_secs(state.heartbeat_secs, 60);

    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(grace_secs)).await;
        let others = {
            let s = state_clone.session.lock().await;
            if s.disconnect_gen == gen && s.mobiles.is_empty() {
                // Notice: Keep desktop text intact! Do not destroy desktop work simply
                // because mobile disconnected. Just notify desktop of mobile leaving.
                let mut txs = Vec::new();
                if let Some(d) = s.desktop.as_ref() {
                    txs.push(d.tx.clone());
                }
                txs
            } else {
                Vec::new()
            }
        };
        for t in others {
            let _ = t.try_send(ServerMsg::PeerLeft);
        }
        publish_manifest(&state_clone).await;
    });
}

async fn publish_manifest(state: &AppState) {
    let (id, connected) = {
        let s = state.session.lock().await;
        (s.id.clone(), s.desktop.is_some() as usize + s.mobiles.len())
    };
    manifest::publish(&id, state.port, connected);
}

async fn broadcast(
    state: &AppState,
    msg: ServerMsg,
    from_role: &str,
    exclude: Option<&mpsc::Sender<ServerMsg>>,
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
        let _ = t.try_send(msg.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_validation_is_strict() {
        let lan = "192.168.1.42";
        let port = 54321;
        assert!(is_valid_ws_origin(None, lan, port));
        assert!(is_valid_ws_origin(
            Some("http://localhost:54321"),
            lan,
            port
        ));
        assert!(is_valid_ws_origin(
            Some("https://localhost:54321"),
            lan,
            port
        ));
        assert!(is_valid_ws_origin(
            Some("http://127.0.0.1:54321"),
            lan,
            port
        ));
        assert!(is_valid_ws_origin(Some("http://[::1]:54321"), lan, port));
        assert!(is_valid_ws_origin(
            Some("http://192.168.1.42:54321"),
            lan,
            port
        ));

        // Reject evil subdomains, mismatched ports, paths, userinfo, non-http schemes
        assert!(!is_valid_ws_origin(
            Some("http://localhost.evil.com:54321"),
            lan,
            port
        ));
        assert!(!is_valid_ws_origin(
            Some("http://localhost:9999"),
            lan,
            port
        ));
        assert!(!is_valid_ws_origin(Some("http://localhost"), lan, port));
        assert!(!is_valid_ws_origin(
            Some("http://192.168.1.42:8080"),
            lan,
            port
        ));
        assert!(!is_valid_ws_origin(
            Some("http://localhost:54321/evil"),
            lan,
            port
        ));
        assert!(!is_valid_ws_origin(
            Some("http://evil@localhost:54321"),
            lan,
            port
        ));
        assert!(!is_valid_ws_origin(
            Some("file://localhost:54321"),
            lan,
            port
        ));
    }

    #[test]
    fn filename_sanitization_and_blocked_extensions() {
        assert_eq!(sanitize_filename("hello%20world.txt"), "hello world.txt");
        assert_eq!(sanitize_filename("..%2F..%2Fetc%2Fpasswd"), "passwd");
        assert_eq!(sanitize_filename(r"C:\Users\me\secret.pdf"), "secret.pdf");
        assert_eq!(sanitize_filename("bad\"name\x00.txt"), "badname.txt");
        assert_eq!(sanitize_filename(".."), "file");
        assert_eq!(sanitize_filename(""), "file");

        assert!(is_blocked_extension("payload.exe"));
        assert!(is_blocked_extension("SCRIPT.PS1"));
        assert!(is_blocked_extension("run.bat"));
        assert!(is_blocked_extension("app.jar"));
        assert!(!is_blocked_extension("photo.png"));
        assert!(!is_blocked_extension("archive.zip"));
    }
}
