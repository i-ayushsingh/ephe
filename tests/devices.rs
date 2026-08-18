//! Live WebSocket test for the v0.7.0 device features: the desktop learns
//! which phones are connected (labels + ids), phones ack received text as
//! "seen" receipts, and the desktop can kick a phone (it gets "kicked", the
//! session ends for everyone else — ephemeral by design).

use std::future::IntoFuture;

use ephe::session::new_session;
use ephe::server::build_router;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

type Ws = tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str) -> Ws {
    tokio_tungstenite::connect_async(url).await.expect("connect").0
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
async fn devices_seen_and_kick_flow() {
    let lan_ip = ephe::net::pick_lan_ip().0;
    if lan_ip == "127.0.0.1" {
        return; // no LAN interface to fake phones from
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
    // the desktop's own join pushes an empty devices list (nothing to show yet)
    let d = recv_json(&mut desktop).await;
    assert_eq!(d["type"], "devices");
    assert_eq!(d["devices"].as_array().unwrap().len(), 0);

    // phone 1 joins -> desktop hears peer_joined then a devices list
    let mut phone1 = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut phone1).await["type"], "init");
    assert_eq!(recv_json(&mut phone1).await["type"], "peer_joined"); // own join
    assert_eq!(recv_json(&mut desktop).await["type"], "peer_joined");
    let devices = recv_json(&mut desktop).await;
    assert_eq!(devices["type"], "devices");
    assert_eq!(devices["devices"].as_array().unwrap().len(), 1);
    let id1 = devices["devices"][0]["id"].as_str().unwrap().to_string();
    assert_eq!(devices["devices"][0]["label"], "phone 1");

    // phone 2 joins -> desktop hears peer_joined then devices[2]
    let mut phone2 = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut phone2).await["type"], "init");
    assert_eq!(recv_json(&mut phone2).await["type"], "peer_joined"); // own join
    assert_eq!(recv_json(&mut phone1).await["type"], "peer_joined"); // phone2's join
    assert_eq!(recv_json(&mut desktop).await["type"], "peer_joined");
    let devices = recv_json(&mut desktop).await;
    assert_eq!(devices["devices"].as_array().unwrap().len(), 2);
    assert_eq!(devices["devices"][1]["label"], "phone 2");

    // phone 1 sends text -> phone 2 gets it and acks -> phone 1 + desktop
    // see "seen phone 2" (the acker itself is excluded)
    phone1
        .send(Message::Text(r#"{"type":"text","text":"hi"}"#.into()))
        .await
        .unwrap();
    let m = recv_json(&mut phone2).await;
    assert_eq!(m["type"], "text");
    assert_eq!(m["text"], "hi");
    phone2
        .send(Message::Text(r#"{"type":"ack"}"#.into()))
        .await
        .unwrap();
    let m = recv_json(&mut phone1).await;
    assert_eq!(m["type"], "seen");
    assert_eq!(m["label"], "phone 2");
    let m = recv_json(&mut desktop).await;
    assert_eq!(m["type"], "text"); // desktop got the "hi" broadcast too
    assert_eq!(m["text"], "hi");
    let m = recv_json(&mut desktop).await;
    assert_eq!(m["type"], "seen");
    assert_eq!(m["label"], "phone 2");

    // desktop kicks phone 1 -> it gets "kicked", everyone else gets peer_left
    desktop
        .send(Message::Text(format!(r#"{{"type":"kick","id":"{id1}"}}"#).into()))
        .await
        .unwrap();
    let m = recv_json(&mut phone1).await;
    assert_eq!(m["type"], "kicked");
    assert_eq!(recv_json(&mut desktop).await["type"], "peer_left");
    assert_eq!(recv_json(&mut phone2).await["type"], "peer_left");
}
