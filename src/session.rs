use std::sync::Arc;
use tokio::sync::Mutex;

use serde::{Deserialize, Serialize};

pub type SharedSession = Arc<Mutex<Session>>;

pub struct Session {
    /// stable per-process session id (PortPeek manifest filename)
    pub id: String,
    pub token: String,
    pub text: String,
    pub desktop: Option<Client>,
    pub mobiles: Vec<Client>,
    /// single-writer mode: when true, only the desktop may edit
    pub locked: bool,
    /// disconnect generation counter for grace period tracking
    pub disconnect_gen: u64,
}

/// Hard cap: one desktop + two mobiles, nothing else.
pub const MAX_MOBILES: usize = 2;

pub struct Client {
    pub id: String,
    pub tx: tokio::sync::mpsc::UnboundedSender<ServerMsg>,
}

/// What the desktop's device list shows: which phones are connected, and how
/// to address them (kick by id, label for display / read receipts).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub id: String,
    pub label: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    #[serde(rename = "init")]
    Init { text: String, locked: bool },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "typing")]
    Typing,
    #[serde(rename = "locked")]
    Locked { locked: bool },
    #[serde(rename = "peer_joined")]
    PeerJoined,
    #[serde(rename = "peer_left")]
    PeerLeft,
    #[serde(rename = "reset")]
    Reset { url: String },
    /// connected phones, for the desktop's device list (kicks included)
    #[serde(rename = "devices")]
    Devices { devices: Vec<DeviceInfo> },
    /// a phone received someone's text — a read receipt
    #[serde(rename = "seen")]
    Seen { label: String },
    /// the desktop removed you from the session
    #[serde(rename = "kicked")]
    Kicked,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "typing")]
    Typing,
    #[serde(rename = "lock")]
    Lock { locked: bool },
    /// desktop-only: remove the phone with this id
    #[serde(rename = "kick")]
    Kick { id: String },
    /// phone-only: "I just received your text" (read receipt)
    #[serde(rename = "ack")]
    Ack,
}

pub fn new_session() -> SharedSession {
    let token = random_token();
    Arc::new(Mutex::new(Session {
        id: random_id(),
        token,
        text: String::new(),
        desktop: None,
        mobiles: Vec::new(),
        locked: false,
        disconnect_gen: 0,
    }))
}

pub fn is_valid_token(session: &Session, token: &str) -> bool {
    constant_time_eq(session.token.as_bytes(), token.as_bytes())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b) {
        diff |= x ^ y;
    }
    diff == 0
}

pub fn random_token() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: [u8; 24] = rng.random();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

pub fn random_id() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let n: u32 = rng.random();
    format!("{:08x}", n)
}
