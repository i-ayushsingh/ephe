pub mod manifest;
pub mod net;
pub mod server;
pub mod session;

use tokio::sync::mpsc;

use crate::session::{Client, ServerMsg, Session};

/// Test-only helpers. Not part of the public API — exists so integration tests
/// can exercise the same state-machine code the WebSocket handler uses.
#[doc(hidden)]
pub mod test_support {
    use super::*;

    pub fn accept(
        s: &mut Session,
        role: &str,
        token: &str,
        tx: mpsc::UnboundedSender<ServerMsg>,
    ) -> Result<(), &'static str> {
        if role == "desktop" {
            if s.desktop.is_some() {
                return Err("desktop already connected");
            }
            s.desktop = Some(Client {
                id: crate::session::random_id(),
                tx,
            });
            Ok(())
        } else {
            if !crate::session::is_valid_token(s, token) {
                return Err("invalid token");
            }
            if s.mobiles.len() >= crate::session::MAX_MOBILES {
                return Err("session full: max 2 mobiles");
            }
            s.mobiles.push(Client {
                id: crate::session::random_id(),
                tx,
            });
            Ok(())
        }
    }

    /// Removes `me` from the session, wipes the text, and returns the senders
    /// of every remaining peer (they get `PeerLeft`). Used by /reset and the
    /// WS handler's on_disconnect.
    pub fn on_disconnect_for(
        s: &mut Session,
        role: &str,
        me: &mpsc::UnboundedSender<ServerMsg>,
    ) -> Vec<mpsc::UnboundedSender<ServerMsg>> {
        if role == "desktop" {
            s.desktop = None;
        } else {
            s.mobiles.retain(|m| !m.tx.same_channel(me));
        }
        let mut txs = Vec::new();
        if role == "desktop" {
            txs.extend(s.mobiles.iter().map(|c| c.tx.clone()));
        } else {
            if let Some(d) = s.desktop.as_ref() {
                txs.push(d.tx.clone());
            }
            txs.extend(s.mobiles.iter().map(|c| c.tx.clone()));
        }
        s.text.clear();
        txs
    }

    /// Rotates the token, disconnects every peer, returns the new desktop URL.
    pub fn reset_for(
        s: &mut Session,
        port: u16,
    ) -> (
        Option<mpsc::UnboundedSender<ServerMsg>>,
        Vec<mpsc::UnboundedSender<ServerMsg>>,
        String,
    ) {
        let dtx = s.desktop.as_ref().map(|c| c.tx.clone());
        let mtxs: Vec<_> = s.mobiles.iter().map(|c| c.tx.clone()).collect();
        s.desktop = None;
        s.mobiles.clear();
        s.text.clear();
        let new_token = crate::session::random_token();
        s.token = new_token.clone();
        let url = format!(
            "http://localhost:{}/?role=desktop&token={}",
            port, new_token
        );
        (dtx, mtxs, url)
    }
}
