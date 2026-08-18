use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// PortPeek discovery manifests — one JSON file per running Ephe session.
/// Location: %LOCALAPPDATA%\Ephe\PortPeek\<session-id>.json
///
/// Windows-only by nature: without LOCALAPPDATA (Linux/macOS) every function
/// here no-ops, so the integration stays strictly optional.

const SCHEMA_VERSION: u32 = 1;
const SESSION_NAME: &str = "Ephe Session";

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    pub application: String,
    pub application_id: String,
    pub session_id: String,
    pub session_name: String,
    pub version: String,
    pub port: u16,
    pub local_url: String,
    /// opens the desktop host UI (?role=desktop); intentionally tokenless —
    /// loopback is trusted (same as the desktop WS join), LAN join still
    /// needs the token, which never appears in the manifest.
    pub host_url: String,
    pub status: String,
    pub connected_devices: usize,
}

fn dir() -> Option<PathBuf> {
    let base = std::env::var("LOCALAPPDATA").ok()?;
    Some(PathBuf::from(base).join("Ephe").join("PortPeek"))
}

fn path_for(session_id: &str) -> Option<PathBuf> {
    dir().map(|d| d.join(format!("{session_id}.json")))
}

/// Create or update the manifest for a running session. No token, no text,
/// no secrets — only what PortPeek needs to discover and open the session.
pub fn publish(session_id: &str, port: u16, connected_devices: usize) {
    let Some(path) = path_for(session_id) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let m = Manifest {
        schema_version: SCHEMA_VERSION,
        application: "Ephe".into(),
        application_id: "ephe".into(),
        session_id: session_id.into(),
        session_name: SESSION_NAME.into(),
        version: env!("CARGO_PKG_VERSION").into(),
        port,
        local_url: format!("http://localhost:{port}"),
        host_url: format!("http://localhost:{port}/?role=desktop"),
        status: "active".into(),
        connected_devices,
    };
    match serde_json::to_string_pretty(&m) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                eprintln!("ephe: portpeek manifest write failed: {e}");
            }
        }
        Err(e) => eprintln!("ephe: portpeek manifest encode failed: {e}"),
    }
}

/// Remove this session's manifest (clean shutdown).
pub fn remove(session_id: &str) {
    if let Some(path) = path_for(session_id) {
        let _ = std::fs::remove_file(path);
    }
}

/// Best-effort stale cleanup: delete manifests whose port no longer answers
/// (crashed sessions). Called once at startup. PortPeek independently
/// validates reachability, so this is a sweep, not a daemon.
// ponytail: a session starting between our probe and its own publish could
// briefly lose its manifest; it republishes immediately, so the window is
// milliseconds and harmless.
pub fn sweep_stale() {
    let Some(d) = dir() else { return };
    let Ok(entries) = std::fs::read_dir(&d) else { return };
    for e in entries.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(m) = serde_json::from_str::<Manifest>(&text) else { continue };
        let Ok(addr) = format!("127.0.0.1:{}", m.port).parse::<SocketAddr>() else { continue };
        let alive =
            std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(200)).is_ok();
        if !alive {
            let _ = std::fs::remove_file(&path);
        }
    }
}