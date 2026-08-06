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
            s.desktop = Some(Client { tx });
            Ok(())
        } else {
            if !crate::session::is_valid_token(s, token) {
                return Err("invalid token");
            }
            if s.mobile.is_some() {
                return Err("mobile already connected");
            }
            s.mobile = Some(Client { tx });
            Ok(())
        }
    }

    /// Returns the other peer's sender, if any, after wiping the disconnecting
    /// side. Used by /reset and the WS handler's on_disconnect.
    pub fn on_disconnect_for(
        s: &mut Session,
        role: &str,
    ) -> Option<mpsc::UnboundedSender<ServerMsg>> {
        if role == "desktop" {
            s.desktop = None;
        } else {
            s.mobile = None;
        }
        let other_tx = if role == "desktop" {
            s.mobile.as_ref().map(|c| c.tx.clone())
        } else {
            s.desktop.as_ref().map(|c| c.tx.clone())
        };
        s.text.clear();
        other_tx
    }

    /// Rotates the token, disconnects both peers, returns the new desktop URL.
    pub fn reset_for(
        s: &mut Session,
        port: u16,
    ) -> (
        Option<mpsc::UnboundedSender<ServerMsg>>,
        Option<mpsc::UnboundedSender<ServerMsg>>,
        String,
    ) {
        let dtx = s.desktop.as_ref().map(|c| c.tx.clone());
        let mtx = s.mobile.as_ref().map(|c| c.tx.clone());
        s.desktop = None;
        s.mobile = None;
        s.text.clear();
        let new_token = crate::session::random_token();
        s.token = new_token.clone();
        let url = format!("http://localhost:{}/?role=desktop&token={}", port, new_token);
        (dtx, mtx, url)
    }
}