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
    let got_peer_left = tokio::time::timeout(Duration::from_secs(12), async {
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
    .expect("desktop received peer_left within 12s");

    assert!(got_peer_left, "desktop must see the silent phone leave");

    drop(phone);
    drop(dsink);
}

#[tokio::test]
async fn mobile_disconnect_grace_period_allows_reconnect_without_session_end() {
    use futures::SinkExt;

    let lan_ip = ephe::net::pick_lan_ip().0;
    if lan_ip == "127.0.0.1" {
        return;
    }
    std::env::set_var("EPHE_HEARTBEAT_SECS", "1");

    let session = new_session();
    let token = session.lock().await.token.clone();
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = build_router(session, port, lan_ip.clone())
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(axum::serve(listener, router).into_future());

    // Desktop connects
    let (desktop, _) =
        tokio_tungstenite::connect_async(format!("ws://127.0.0.1:{port}/ws?token={token}"))
            .await
            .expect("desktop connect");
    let (mut dsink, mut dstream) = desktop.split();
    let _ = dstream.next().await.unwrap().unwrap();

    // Desktop types text
    dsink
        .send(Message::Text(
            r#"{"type":"text","text":"keep me alive"}"#.into(),
        ))
        .await
        .unwrap();

    // Mobile connects
    let (phone1, _) =
        tokio_tungstenite::connect_async(format!("ws://{lan_ip}:{port}/ws?token={token}"))
            .await
            .expect("phone1 connect");
    let (_p1_sink, mut p1_stream) = phone1.split();
    let p1_init = p1_stream.next().await.unwrap().unwrap();
    assert!(p1_init.into_text().unwrap().contains("keep me alive"));

    // Phone 1 disconnects (simulating browser minimized / switched app)
    drop(_p1_sink);
    drop(p1_stream);

    // Give server time to process disconnect
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Check desktop: desktop should receive Devices update (0 phones), but NOT peer_left
    let mut got_peer_left = false;
    let check_deadline = tokio::time::Instant::now() + Duration::from_millis(500);
    while tokio::time::Instant::now() < check_deadline {
        if let Ok(Some(Ok(msg))) =
            tokio::time::timeout(Duration::from_millis(100), dstream.next()).await
        {
            if let Message::Text(t) = msg {
                if t.contains(r#""type":"peer_left""#) {
                    got_peer_left = true;
                }
            }
        }
    }
    assert!(
        !got_peer_left,
        "desktop must NOT get peer_left when phone minimizes/disconnects temporarily"
    );

    // Phone reconnects before grace period expires
    let (phone2, _) =
        tokio_tungstenite::connect_async(format!("ws://{lan_ip}:{port}/ws?token={token}"))
            .await
            .expect("phone2 reconnect");
    let (_p2_sink, mut p2_stream) = phone2.split();
    let p2_init = p2_stream
        .next()
        .await
        .unwrap()
        .unwrap()
        .into_text()
        .unwrap();
    assert!(
        p2_init.contains("keep me alive"),
        "reconnected phone must receive the preserved session text: {p2_init}"
    );

    drop(dsink);
}
