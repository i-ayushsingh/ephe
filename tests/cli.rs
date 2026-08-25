use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_ephe")
}

/// Each test gets its own LOCALAPPDATA so runs never collide.
fn appdata(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("ephe-cli-test-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn spawn(args: &[&str], appdata: &std::path::Path) -> Child {
    Command::new(bin())
        .args(args)
        .env("LOCALAPPDATA", appdata)
        .env("EPHE_NO_BROWSER", "1")
        .spawn()
        .unwrap()
}

fn wait_for_manifests(appdata: &std::path::Path, count: usize) -> Vec<PathBuf> {
    let pp = appdata.join("Ephe").join("PortPeek");
    let start = Instant::now();
    loop {
        let files: Vec<PathBuf> = std::fs::read_dir(&pp)
            .map(|rd| rd.flatten().map(|e| e.path()).collect())
            .unwrap_or_default();
        if files.len() >= count {
            return files;
        }
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for {count} manifests"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn kill(mut child: Child) {
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn new_session_flag_starts_and_publishes_manifest() {
    let appdata = appdata("flag");
    let child = spawn(&["--new-session"], &appdata);

    let files = wait_for_manifests(&appdata, 1);
    let m: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&files[0]).unwrap()).unwrap();
    assert_eq!(m["applicationId"], "ephe");
    assert_eq!(m["sessionName"], "Ephe Session");
    let port = m["port"].as_u64().unwrap();
    assert!(port > 0, "a real listening port is reported");
    assert!(m["sessionId"].as_str().unwrap().len() >= 8);
    assert!(
        TcpStream::connect_timeout(
            &format!("127.0.0.1:{port}").parse().unwrap(),
            Duration::from_secs(2)
        )
        .is_ok(),
        "the reported port is actually listening"
    );

    kill(child);
}

#[test]
fn plain_launch_unchanged_also_publishes() {
    let appdata = appdata("plain");
    let child = spawn(&[], &appdata);
    let files = wait_for_manifests(&appdata, 1);
    assert_eq!(files.len(), 1);
    kill(child);
}

#[test]
fn two_invocations_create_independent_sessions() {
    let appdata = appdata("two");
    let c1 = spawn(&["--new-session"], &appdata);
    let c2 = spawn(&["--new-session"], &appdata);

    let files = wait_for_manifests(&appdata, 2);
    let mut ports = Vec::new();
    let mut ids = Vec::new();
    for f in &files {
        let m: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap();
        ports.push(m["port"].as_u64().unwrap());
        ids.push(m["sessionId"].as_str().unwrap().to_string());
    }
    ports.sort();
    ports.dedup();
    ids.sort();
    ids.dedup();
    assert_eq!(ports.len(), 2, "two different ports");
    assert_eq!(ids.len(), 2, "two different session ids");

    kill(c1);
    kill(c2);
}

#[test]
fn invalid_flag_fails_cleanly_without_manifest() {
    let appdata = appdata("bad");
    let out = Command::new(bin())
        .arg("--bogus")
        .env("LOCALAPPDATA", &appdata)
        .env("EPHE_NO_BROWSER", "1")
        .output()
        .unwrap();
    assert!(!out.status.success(), "non-zero exit code");
    let pp = appdata.join("Ephe").join("PortPeek");
    let leftover = std::fs::read_dir(&pp).map(|rd| rd.count()).unwrap_or(0);
    assert_eq!(leftover, 0, "no manifest, no stale state");
}
