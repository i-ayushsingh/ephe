//! Integration tests for the session/state-machine layer.
//!
//! These exercise the same accept / on_disconnect / reset code paths the
//! WebSocket handler uses, against synthetic channel senders — no real socket
//! or network required.

use ephe::session::{is_valid_token, new_session, random_token, ClientMsg, ServerMsg};
use ephe::test_support::{accept, on_disconnect_for, reset_for};
use tokio::sync::mpsc;

#[test]
fn token_round_trip() {
    let s = new_session();
    let token = s.blocking_lock().token.clone();
    assert!(is_valid_token(&s.blocking_lock(), &token));
    assert!(!is_valid_token(&s.blocking_lock(), ""));
    assert!(!is_valid_token(&s.blocking_lock(), "deadbeef"));
}

#[test]
fn random_token_is_unique_ish() {
    let a = random_token();
    let b = random_token();
    assert_eq!(a.len(), 48);
    assert_eq!(b.len(), 48);
    assert_ne!(a, b);
}

#[tokio::test]
async fn desktop_join_then_rejoin_is_rejected() {
    let session = new_session();
    let (tx1, _rx1) = mpsc::unbounded_channel::<ServerMsg>();
    let (tx2, _rx2) = mpsc::unbounded_channel::<ServerMsg>();

    let mut s = session.lock().await;
    assert!(accept(&mut s, "desktop", "", tx1).is_ok());
    let err = accept(&mut s, "desktop", "", tx2).unwrap_err();
    assert_eq!(err, "desktop already connected");
}

#[tokio::test]
async fn mobile_join_with_wrong_token_is_rejected() {
    let session = new_session();
    let token = session.lock().await.token.clone();
    let (tx, _rx) = mpsc::unbounded_channel::<ServerMsg>();

    let mut s = session.lock().await;
    let err = accept(&mut s, "mobile", "wrong", tx).unwrap_err();
    assert_eq!(err, "invalid token");

    let (tx, _rx) = mpsc::unbounded_channel::<ServerMsg>();
    assert!(accept(&mut s, "mobile", &token, tx).is_ok());
}

#[tokio::test]
async fn mobile_join_twice_is_rejected() {
    let session = new_session();
    let token = session.lock().await.token.clone();
    let (tx1, _rx1) = mpsc::unbounded_channel::<ServerMsg>();
    let (tx2, _rx2) = mpsc::unbounded_channel::<ServerMsg>();

    let mut s = session.lock().await;
    assert!(accept(&mut s, "mobile", &token, tx1).is_ok());
    let err = accept(&mut s, "mobile", &token, tx2).unwrap_err();
    assert_eq!(err, "mobile already connected");
}

#[tokio::test]
async fn disconnect_wipes_text_and_passes_peer_left_to_other() {
    let session = new_session();
    let token = session.lock().await.token.clone();
    let (dtx, _drx) = mpsc::unbounded_channel::<ServerMsg>();
    let (mtx, mut mrx) = mpsc::unbounded_channel::<ServerMsg>();

    {
        let mut s = session.lock().await;
        accept(&mut s, "desktop", "", dtx).unwrap();
        accept(&mut s, "mobile", &token, mtx).unwrap();
        s.text = "secret".into();
    }

    // desktop leaves -> mobile should get peer_left, text wiped, mobile still in
    let peer_left_tx = {
        let mut s = session.lock().await;
        let other = on_disconnect_for(&mut s, "desktop");
        assert!(other.is_some(), "mobile should still be present");
        assert!(s.text.is_empty(), "text must be wiped on disconnect");
        assert!(s.desktop.is_none());
        assert!(s.mobile.is_some());
        other.unwrap()
    };
    peer_left_tx.send(ServerMsg::PeerLeft).unwrap();
    let got = mrx.recv().await.unwrap();
    assert!(matches!(got, ServerMsg::PeerLeft));

    // mobile leaves too -> both gone, no peer_left target
    let peer_left_tx = {
        let mut s = session.lock().await;
        let other = on_disconnect_for(&mut s, "mobile");
        assert!(other.is_none());
        assert!(s.desktop.is_none());
        assert!(s.mobile.is_none());
        other
    };
    assert!(peer_left_tx.is_none());
}

#[tokio::test]
async fn reset_rotates_token_and_disconnects_clients() {
    let session = new_session();
    let old_token = session.lock().await.token.clone();
    let (dtx, mut drx) = mpsc::unbounded_channel::<ServerMsg>();
    let (mtx, mut mrx) = mpsc::unbounded_channel::<ServerMsg>();

    {
        let mut s = session.lock().await;
        accept(&mut s, "desktop", "", dtx).unwrap();
        accept(&mut s, "mobile", &old_token, mtx).unwrap();
        s.text = "secret".into();
    }

    let (dtx, mtx, url) = {
        let mut s = session.lock().await;
        let (d, m, url) = reset_for(&mut s, 12345);
        assert!(d.is_some());
        assert!(m.is_some());
        assert_ne!(s.token, old_token, "token must rotate");
        assert!(s.text.is_empty());
        assert!(s.desktop.is_none());
        assert!(s.mobile.is_none());
        assert!(url.contains("role=desktop"));
        assert!(url.contains(&s.token));
        (d, m, url)
    };

    dtx.unwrap().send(ServerMsg::Reset { url: url.clone() }).unwrap();
    mtx.unwrap().send(ServerMsg::PeerLeft).unwrap();

    let got = drx.recv().await.unwrap();
    match got {
        ServerMsg::Reset { url: u } => assert_eq!(u, url),
        _ => panic!("expected Reset, got {:?}", got),
    }
    let got = mrx.recv().await.unwrap();
    assert!(matches!(got, ServerMsg::PeerLeft));
}

// --- compile-time sanity: messages round-trip through serde ---

#[test]
fn server_msg_serializes_with_type_tag() {
    let s = ServerMsg::Init { text: "hi".into() };
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"type":"init","text":"hi"}"#);

    let s = ServerMsg::PeerJoined;
    assert_eq!(serde_json::to_string(&s).unwrap(), r#"{"type":"peer_joined"}"#);

    let s = ServerMsg::Reset { url: "http://localhost:1234/?token=abc".into() };
    let j = serde_json::to_string(&s).unwrap();
    assert!(j.contains(r#""type":"reset""#));
    assert!(j.contains(r#""url":"http://localhost:1234/?token=abc""#));
}

#[test]
fn client_msg_deserializes() {
    let j = r#"{"type":"text","text":"hello"}"#;
    let m: ClientMsg = serde_json::from_str(j).unwrap();
    match m {
        ClientMsg::Text { text } => assert_eq!(text, "hello"),
    }
}