use std::net::{Ipv4Addr, SocketAddr};

use rand::Rng;
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug)]
pub enum IfKind {
    Tunnel,
    Other,
    Wifi,
}

pub fn pick_port() -> u16 {
    if let Ok(v) = std::env::var("EPHE_PORT") {
        if let Ok(p) = v.parse::<u16>() {
            if p >= 1024 {
                return p;
            }
            eprintln!("ephe: ignoring EPHE_PORT={} (need >= 1024)", p);
        }
    }
    let mut rng = rand::rng();
    rng.random_range(40000..50000)
}

/// Bind a TCP listener with retry logic across the 40000..50000 port range,
/// avoiding Hyper-V / WSL dynamic port exclusion blocks on Windows.
pub async fn bind_listener() -> Result<(TcpListener, u16), std::io::Error> {
    if let Ok(v) = std::env::var("EPHE_PORT") {
        if let Ok(p) = v.parse::<u16>() {
            if p >= 1024 {
                let addr = SocketAddr::from(([0, 0, 0, 0], p));
                let l = TcpListener::bind(addr).await?;
                return Ok((l, p));
            }
            eprintln!("ephe: ignoring EPHE_PORT={} (need >= 1024)", p);
        }
    }
    let mut rng = rand::rng();
    let mut last_err = None;
    for _ in 0..20 {
        let p = rng.random_range(40000..50000);
        let addr = SocketAddr::from(([0, 0, 0, 0], p));
        match TcpListener::bind(addr).await {
            Ok(l) => return Ok((l, p)),
            Err(e) => last_err = Some(e),
        }
    }
    let addr = SocketAddr::from(([0, 0, 0, 0], 0));
    let l = TcpListener::bind(addr)
        .await
        .map_err(|e| last_err.unwrap_or(e))?;
    let p = l.local_addr()?.port();
    Ok((l, p))
}

pub struct Candidate {
    pub name: String,
    pub ip: String,
    pub kind: IfKind,
}

pub fn pick_lan_ip() -> (String, Vec<Candidate>) {
    if let Ok(v) = std::env::var("EPHE_IP") {
        if !v.is_empty() {
            return (v, list_ips());
        }
    }
    let all = list_ips();
    let picked = all
        .iter()
        .max_by_key(|c| score_candidate(c))
        .map(|c| {
            eprintln!("ephe: using {} ({:?})", c.ip, c.kind);
            c.ip.clone()
        })
        .unwrap_or_else(|| "127.0.0.1".to_string());
    (picked, all)
}

fn list_ips() -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Ok(v) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in v {
            let std::net::IpAddr::V4(v4) = ip else {
                continue;
            };
            if v4.is_loopback() {
                continue;
            }
            let kind = classify_iface(&name);
            out.push(Candidate {
                name,
                ip: v4.to_string(),
                kind,
            });
        }
    }
    out
}

fn classify_iface(name: &str) -> IfKind {
    let n = name.to_lowercase();
    let tunnel = [
        "nordlynx",
        "tailscale",
        "wireguard",
        "zerotier",
        "hamachi",
        "tap-",
        "tun",
        "vpn",
        "wg",
        "utun",
        "ppp",
        "wan",
    ];
    if tunnel.iter().any(|k| n.contains(k)) {
        return IfKind::Tunnel;
    }
    let wifi = ["wi-fi", "wifi", "wlan", "wireless", "802.11"];
    if wifi.iter().any(|k| n.contains(k)) {
        return IfKind::Wifi;
    }
    IfKind::Other
}

fn score_candidate(c: &Candidate) -> (u8, u8) {
    let kind_score = match c.kind {
        IfKind::Wifi => 2,
        IfKind::Other => 1,
        IfKind::Tunnel => 0,
    };
    let v4: Ipv4Addr = c.ip.parse().unwrap_or(Ipv4Addr::UNSPECIFIED);
    let range = if v4.is_private() {
        2
    } else if v4.is_link_local() {
        0
    } else {
        1
    };
    (range, kind_score)
}
