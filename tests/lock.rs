//! Live WebSocket test for the lock pad (single-writer mode): only the
//! desktop may lock/unlock; phones see the state change and their own lock
//! attempts are ignored.

use std::future::IntoFuture;
use std::time::Duration;

use ephe::server::build_router;
use ephe::session::new_session;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str) -> Ws {
    tokio_tungstenite::connect_async(url)
        .await
        .expect("connect")
        .0
}

async fn recv_json(ws: &mut Ws) -> serde_json::Value {
    loop {
        let msg = ws.next().await.expect("frame").expect("ws ok");
        match msg {
            Message::Text(t) => return serde_json::from_str(&t).expect("json"),
            _ => continue, // skip pings/pongs/close
        }
    }
}

#[tokio::test]
async fn desktop_lock_is_relayed_and_phone_lock_is_ignored() {
    let lan_ip = ephe::net::pick_lan_ip().0;
    if lan_ip == "127.0.0.1" {
        return; // no LAN interface to fake a phone from
    }

    let session = new_session();
    let token = session.lock().await.token.clone();
    let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let router = build_router(session, port, lan_ip.clone())
        .into_make_service_with_connect_info::<std::net::SocketAddr>();
    tokio::spawn(axum::serve(listener, router).into_future());

    let mut desktop = connect(&format!("ws://127.0.0.1:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut desktop).await["type"], "init");
    assert_eq!(recv_json(&mut desktop).await["type"], "devices"); // own join, empty list

    let mut phone = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut phone).await["type"], "init");
    let m = recv_json(&mut phone).await; // joiner hears its own join broadcast
    assert_eq!(m["type"], "peer_joined");
    let m = recv_json(&mut desktop).await; // desktop hears the phone's join
    assert_eq!(m["type"], "peer_joined");
    assert_eq!(recv_json(&mut desktop).await["type"], "devices"); // phone 1 listed

    // desktop locks -> phone sees it
    desktop
        .send(Message::Text(r#"{"type":"lock","locked":true}"#.into()))
        .await
        .unwrap();
    let m = recv_json(&mut phone).await;
    assert_eq!(m["type"], "locked");
    assert_eq!(m["locked"], true);

    // phone tries to lock -> ignored, no locked broadcast reaches the desktop
    phone
        .send(Message::Text(r#"{"type":"lock","locked":true}"#.into()))
        .await
        .unwrap();
    let nothing = tokio::time::timeout(Duration::from_millis(700), desktop.next()).await;
    assert!(
        nothing.is_err(),
        "desktop must not hear a phone's lock attempt"
    );

    // desktop unlocks -> phone sees it
    desktop
        .send(Message::Text(r#"{"type":"lock","locked":false}"#.into()))
        .await
        .unwrap();
    let m = recv_json(&mut phone).await;
    assert_eq!(m["type"], "locked");
    assert_eq!(m["locked"], false);
}
