use std::sync::Arc;
use tokio::sync::Mutex;

use serde::{Deserialize, Serialize};

pub type SharedSession = Arc<Mutex<Session>>;

pub struct Session {
    pub token: String,
    pub text: String,
    pub desktop: Option<Client>,
    pub mobiles: Vec<Client>,
}

/// Hard cap: one desktop + two mobiles, nothing else.
pub const MAX_MOBILES: usize = 2;

pub struct Client {
    pub tx: tokio::sync::mpsc::UnboundedSender<ServerMsg>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ServerMsg {
    #[serde(rename = "init")]
    Init { text: String },
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "peer_joined")]
    PeerJoined,
    #[serde(rename = "peer_left")]
    PeerLeft,
    #[serde(rename = "reset")]
    Reset { url: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClientMsg {
    #[serde(rename = "text")]
    Text { text: String },
}

pub fn new_session() -> SharedSession {
    let token = random_token();
    Arc::new(Mutex::new(Session {
        token,
        text: String::new(),
        desktop: None,
        mobiles: Vec::new(),
    }))
}

pub fn is_valid_token(session: &Session, token: &str) -> bool {
    session.token.as_bytes() == token.as_bytes()
}

pub fn random_token() -> String {
    use rand::Rng;
    let mut rng = rand::rng();
    let bytes: [u8; 24] = rng.random();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}