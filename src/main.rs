use std::net::SocketAddr;

use axum::serve;
use ephe::{manifest, net, server, session};
use tokio::net::TcpListener;

/// CLI contract for PortPeek. Today only `--new-session`, which is exactly
/// what a plain `ephe.exe` launch already does: create a new normal session,
/// start the server, publish the manifest, open the browser. Future commands
/// (e.g. `--open-session <id>`) slot into this match.
fn parse_args(args: &[String]) -> Result<(), String> {
    match args {
        [] => Ok(()),
        [flag] if flag == "--new-session" => Ok(()),
        [flag] => Err(format!("unknown argument: {flag}")),
        _ => Err("too many arguments".into()),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(e) = parse_args(&args) {
        eprintln!("ephe: {e}");
        eprintln!("usage: ephe.exe [--new-session]");
        std::process::exit(1);
    }

    let session = session::new_session();
    let session_id = { session.lock().await.id.clone() };

    let port = net::pick_port();
    let (lan_ip, candidates) = net::pick_lan_ip();

    let token = { session.lock().await.token.clone() };
    let desktop_url = format!("http://localhost:{}/?role=desktop&token={}", port, token);
    let mobile_url = format!("http://{}:{}/?token={}", lan_ip, port, token);

    let local_addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(local_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("ephe: failed to bind port {}: {}", port, e);
            std::process::exit(1);
        }
    };

    // PortPeek discovery: clear crash leftovers, then announce this session.
    manifest::sweep_stale();
    manifest::publish(&session_id, port, 0);

    let router = server::build_router(session, port, lan_ip.clone());

    let server_task = tokio::spawn(async move {
        let svc = router.into_make_service_with_connect_info::<SocketAddr>();
        let _ = serve(listener, svc)
            .with_graceful_shutdown(async move {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await;
    });

    if std::env::var("EPHE_NO_BROWSER").is_err() {
        if let Err(e) = webbrowser::open(&desktop_url) {
            eprintln!("ephe: failed to open browser: {}", e);
            eprintln!("ephe: open this URL manually: {}", desktop_url);
        }
    }

    println!("ephe: session started");
    println!("ephe: desktop: {}", desktop_url);
    println!("ephe: mobile : {}", mobile_url);
    if candidates.len() > 1 {
        println!("ephe: other interfaces available:");
        for c in &candidates {
            if c.ip != lan_ip {
                println!(
                    "        {}  http://{}:{}/?token={}",
                    c.name, c.ip, port, token
                );
            }
        }
        println!("ephe: override with EPHE_IP=192.168.x.x if the phone is on a different network");
    }
    println!("ephe: ctrl+c to quit");

    let _ = server_task.await;
    manifest::remove(&session_id);
    println!("ephe: stopped");
}
