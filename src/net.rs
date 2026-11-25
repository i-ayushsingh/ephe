use std::net::{Ipv4Addr, SocketAddr};

use rand::Rng;
use tokio::net::TcpListener;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IfKind {
    Virtual,
    Tunnel,
    Other,
    Ethernet,
    Wifi,
}

pub const DEFAULT_PORT: u16 = 42380;

pub fn pick_port() -> u16 {
    if let Ok(v) = std::env::var("EPHE_PORT") {
        if let Ok(p) = v.parse::<u16>() {
            if p >= 1024 {
                return p;
            }
            eprintln!("ephe: ignoring EPHE_PORT={} (need >= 1024)", p);
        }
    }
    DEFAULT_PORT
}

/// Bind a TCP listener. First attempts a predictable port (DEFAULT_PORT=42380) so
/// Windows Firewall rules remain consistent across runs. Falls back across the
/// 40000..50000 port range if 42380 is already bound.
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

    // Try default port first for firewall rule stability
    let addr = SocketAddr::from(([0, 0, 0, 0], DEFAULT_PORT));
    if let Ok(l) = TcpListener::bind(addr).await {
        return Ok((l, DEFAULT_PORT));
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

#[derive(Clone, Debug)]
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
        .filter(|c| {
            let score = score_candidate(c);
            score.0 > 0 && score.1 > 0
        })
        .map(|c| {
            eprintln!("ephe: using LAN IP {} on {} ({:?})", c.ip, c.name, c.kind);
            c.ip.clone()
        })
        .unwrap_or_else(|| {
            eprintln!("ephe: no active LAN Wi-Fi interface detected, defaulting to 127.0.0.1");
            "127.0.0.1".to_string()
        });
    (picked, all)
}

pub fn list_ips() -> Vec<Candidate> {
    let mut out = Vec::new();
    if let Ok(v) = local_ip_address::list_afinet_netifas() {
        for (name, ip) in v {
            let std::net::IpAddr::V4(v4) = ip else {
                continue;
            };
            if v4.is_loopback() || v4.is_unspecified() || v4.is_link_local() {
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

    // Virtual adapters (Hyper-V, WSL, VirtualBox, VMware, Docker) must never be chosen
    let virtual_ifaces = [
        "vethernet",
        "wsl",
        "hyper-v",
        "virtualbox",
        "vmware",
        "docker",
        "default switch",
        "host-only",
        "bluetooth",
        "p2p",
        "teredo",
        "isatap",
    ];
    if virtual_ifaces.iter().any(|k| n.contains(k)) {
        return IfKind::Virtual;
    }

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

    let eth = ["ethernet", "eth", "local area connection", "lan"];
    if eth.iter().any(|k| n.contains(k)) {
        return IfKind::Ethernet;
    }

    IfKind::Other
}

fn score_candidate(c: &Candidate) -> (u8, u8) {
    let kind_score = match c.kind {
        IfKind::Wifi => 4,
        IfKind::Ethernet => 3,
        IfKind::Other => 2,
        IfKind::Tunnel => 0,
        IfKind::Virtual => 0,
    };

    let v4: Ipv4Addr = c.ip.parse().unwrap_or(Ipv4Addr::UNSPECIFIED);
    let octets = v4.octets();

    // Prioritize standard router LAN subnets (192.168.x.x, excluding Windows Hotspot 192.168.137.1)
    let range_score =
        if octets[0] == 192 && octets[1] == 168 && !(octets[2] == 137 && octets[3] == 1) {
            3
        } else if v4.is_private() {
            2
        } else if v4.is_link_local() || v4.is_loopback() || v4.is_unspecified() {
            0
        } else {
            1
        };

    (range_score, kind_score)
}
