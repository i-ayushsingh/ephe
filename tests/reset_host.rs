//! The tokenless hostUrl story: a desktop host opened via
//! `http://localhost:<port>/?role=desktop` (no token in the URL) must still
//! be able to reset/start a new session, while LAN clients keep needing the
//! session token. Loopback is trusted exactly like the desktop WS join.

use std::future::IntoFuture;
use std::io::{Read, Write};
use std::time::Duration;

use ephe::session::new_session;
use ephe::server::build_router;
use tokio::net::TcpListener;

fn raw_http(host: &str, port: u16, request: &str) -> (u16, String) {
    let mut s = std::net::TcpStream::connect((host, port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
    s.write_all(request.as_bytes()).unwrap();
    let mut buf = String::new();
    let _ = s.read_to_string(&mut buf);
    let status = buf
        .split_whitespace()
        .nth(1)
        .map(|c| c.parse().unwrap())
        .unwrap_or(0);
    (status, buf)
}

async fn spawn_server() -> (u16, String, String) {
    let session = new_session();
    let token = session.lock().await.token.clone();
    let lan_ip = ephe::net::pick_lan_ip().0;
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = build_router(session, port, lan_ip.clone())
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(axum::serve(listener, router).into_future());
    (port, token, lan_ip)
}

#[tokio::test(flavor = "multi_thread")]
async fn loopback_reset_without_token_is_allowed() {
    let (port, _, _) = spawn_server().await;
    let (status, _) = raw_http(
        "127.0.0.1",
        port,
        &format!("POST /reset?token= HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status, 200, "tokenless host UI can reset");
}

#[tokio::test(flavor = "multi_thread")]
async fn loopback_reset_with_garbage_token_is_allowed() {
    let (port, _, _) = spawn_server().await;
    let (status, _) = raw_http(
        "127.0.0.1",
        port,
        &format!("POST /reset?token=garbage HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status, 200, "loopback trust, token irrelevant");
}

#[tokio::test(flavor = "multi_thread")]
async fn lan_reset_without_token_is_forbidden() {
    let (port, _, lan_ip) = spawn_server().await;
    if lan_ip == "127.0.0.1" {
        return; // no LAN interface to fake a phone from
    }
    let (status, _) = raw_http(
        &lan_ip,
        port,
        &format!("POST /reset?token= HTTP/1.1\r\nHost: {lan_ip}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status, 403, "LAN cannot use the host-only reset privilege");
}

#[tokio::test(flavor = "multi_thread")]
async fn lan_reset_with_valid_token_is_allowed() {
    let (port, token, lan_ip) = spawn_server().await;
    if lan_ip == "127.0.0.1" {
        return;
    }
    let (status, _) = raw_http(
        &lan_ip,
        port,
        &format!("POST /reset?token={token} HTTP/1.1\r\nHost: {lan_ip}\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status, 200, "authenticated LAN secondary device still resets");
}

#[tokio::test(flavor = "multi_thread")]
async fn host_url_serves_host_ui_markup() {
    let (port, _, _) = spawn_server().await;
    let (status, body) = raw_http(
        "127.0.0.1",
        port,
        &format!("GET /?role=desktop HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"),
    );
    assert_eq!(status, 200);
    // the host UI markup (QR panel + host-only controls) is what the
    // client-side role switch reveals; the switch itself is driven by the
    // `role=desktop` query parameter that hostUrl carries.
    assert!(body.contains("qr-panel"), "QR panel markup served");
    assert!(body.contains("lock-btn"), "host lock control served");
    assert!(body.contains("reset-btn"), "host reset control served");
}
