//! Live WebSocket test for the heartbeat: a phone that goes silent (app
//! force-killed, Wi-Fi dropped) must be detected and reaped, so the desktop
//! gets `peer_left` instead of a phantom "2 phones connected" forever.

use std::future::IntoFuture;
use std::time::Duration;

use ephe::server::build_router;
use ephe::session::new_session;
use futures::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn silent_phone_is_reaped_and_desktop_gets_peer_left() {
    let lan_ip = ephe::net::pick_lan_ip().0;
    if lan_ip == "127.0.0.1" {
        return; // no LAN interface to fake a phone from; desktop role is loopback-only
    }
    std::env::set_var("EPHE_HEARTBEAT_SECS", "1");

    let session = new_session();
    let token = session.lock().await.token.clone();
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = build_router(session, port, lan_ip.clone())
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(axum::serve(listener, router).into_future());

    // desktop connects on loopback -> desktop role
    let (desktop, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?token={token}"))
            .await
            .expect("desktop connect");
    let (dsink, mut dstream) = desktop.split();
    let init = dstream
        .next()
        .await
        .expect("desktop init frame")
        .expect("desktop init ok");
    assert!(init.into_text().unwrap().contains(r#""type":"init""#));

    // phone connects via the LAN IP -> mobile role, then goes silent: we never
    // read, so the server's pings are never ponged
    let phone = tokio_tungstenite::connect_async(format!("ws://{lan_ip}:{port}/ws?token={token}"))
        .await
        .expect("phone connect")
        .0;

    // desktop reads (and auto-pongs) so it survives; collect until peer_left
    let got_peer_left = tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let msg = dstream
                .next()
                .await
                .expect("desktop frame")
                .expect("desktop ok");
            match msg {
                Message::Text(t) if t.to_string().contains(r#""type":"peer_left""#) => break true,
                Message::Text(t) if t.to_string().contains(r#""type":"peer_joined""#) => continue,
                _ => continue,
            }
        }
    })
    .await
    .expect("desktop received peer_left within 8s");

    assert!(got_peer_left, "desktop must see the silent phone leave");

    drop(phone);
    drop(dsink);
}
