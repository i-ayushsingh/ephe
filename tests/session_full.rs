//! Live WebSocket test for the 3rd-mobile inline rejection: two phones fit,
//! a third is refused with an "error" message naming the reason before the
//! socket closes (the client shows that reason as a persistent notice).

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
async fn third_mobile_is_rejected_with_reason() {
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

    let mut phone1 = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut phone1).await["type"], "init");
    let _ = recv_json(&mut phone1).await; // own peer_joined

    let mut phone2 = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    assert_eq!(recv_json(&mut phone2).await["type"], "init");
    let _ = recv_json(&mut phone2).await; // own peer_joined

    let mut phone3 = connect(&format!("ws://{lan_ip}:{port}/ws?token={token}")).await;
    let m = recv_json(&mut phone3).await;
    assert_eq!(m["type"], "error");
    assert!(m["reason"].as_str().unwrap().contains("session full"));
}