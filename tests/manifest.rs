use std::io::Read;

use ephe::manifest;

/// Manifest functions read the global LOCALAPPDATA env var, so tests must
/// run one at a time.
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Each test gets its own LOCALAPPDATA so tests never collide (env is global).
fn temp_appdata(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("ephe-manifest-test-{}-{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(d.join("Ephe").join("PortPeek")).unwrap();
    std::env::set_var("LOCALAPPDATA", &d);
    d
}

fn read_manifest(dir: &std::path::Path, session_id: &str) -> serde_json::Value {
    let p = dir
        .join("Ephe")
        .join("PortPeek")
        .join(format!("{session_id}.json"));
    let mut f = std::fs::File::open(&p).expect("manifest file exists");
    let mut s = String::new();
    f.read_to_string(&mut s).unwrap();
    serde_json::from_str(&s).unwrap()
}

#[test]
fn creates_manifest_with_all_fields() {
    let _guard = LOCK.lock().unwrap();
    let dir = temp_appdata("create");
    manifest::publish("abc123", 5500, 2);

    let m = read_manifest(&dir, "abc123");
    assert_eq!(m["schemaVersion"], 1);
    assert_eq!(m["application"], "Ephe");
    assert_eq!(m["applicationId"], "ephe");
    assert_eq!(m["sessionId"], "abc123");
    assert_eq!(m["sessionName"], "Ephe Session");
    assert_eq!(m["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(m["port"], 5500);
    assert_eq!(m["localUrl"], "http://localhost:5500");
    assert_eq!(m["hostUrl"], "http://localhost:5500/?role=desktop");
    assert!(
        m["hostUrl"].as_str().unwrap().contains("role=desktop"),
        "hostUrl opens the desktop host UI"
    );
    assert!(
        !m["hostUrl"].as_str().unwrap().contains("token"),
        "hostUrl intentionally carries no session token"
    );
    assert_eq!(m["status"], "active");
    assert_eq!(m["connectedDevices"], 2);
    assert!(m.get("token").is_none(), "no secrets in the manifest");
}

#[test]
fn update_overwrites_port_and_device_count() {
    let _guard = LOCK.lock().unwrap();
    let dir = temp_appdata("update");
    manifest::publish("s1", 5001, 0);
    manifest::publish("s1", 5002, 3);

    let m = read_manifest(&dir, "s1");
    assert_eq!(m["port"], 5002);
    assert_eq!(m["localUrl"], "http://localhost:5002");
    assert_eq!(m["connectedDevices"], 3);
    assert_eq!(m["sessionId"], "s1");
}

#[test]
fn remove_deletes_manifest() {
    let _guard = LOCK.lock().unwrap();
    let dir = temp_appdata("remove");
    manifest::publish("s1", 5001, 0);
    manifest::remove("s1");

    assert!(!dir.join("Ephe").join("PortPeek").join("s1.json").exists());
}

#[test]
fn no_op_without_localappdata() {
    let _guard = LOCK.lock().unwrap();
    let saved = std::env::var("LOCALAPPDATA").ok();
    std::env::remove_var("LOCALAPPDATA");
    manifest::publish("s1", 5001, 0); // must not panic
    manifest::remove("s1");
    manifest::sweep_stale();
    if let Some(v) = saved {
        std::env::set_var("LOCALAPPDATA", v);
    }
}

#[test]
fn sweep_removes_dead_session_keeps_live_one() {
    let _guard = LOCK.lock().unwrap();
    let dir = temp_appdata("sweep");

    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let live_port = listener.local_addr().unwrap().port();
    manifest::publish("dead", 59998, 0); // nothing listens on 59998
    manifest::publish("live", live_port, 0);

    manifest::sweep_stale();

    let pp = dir.join("Ephe").join("PortPeek");
    assert!(!pp.join("dead.json").exists(), "dead session swept");
    assert!(pp.join("live.json").exists(), "live session kept");

    drop(listener);
}

#[test]
fn three_sessions_coexist() {
    let _guard = LOCK.lock().unwrap();
    let dir = temp_appdata("multi");
    manifest::publish("aaa", 5001, 0);
    manifest::publish("bbb", 5002, 1);
    manifest::publish("ccc", 5003, 2);

    let pp = dir.join("Ephe").join("PortPeek");
    let files: Vec<_> = std::fs::read_dir(&pp)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(files.len(), 3);

    assert_eq!(read_manifest(&dir, "aaa")["port"], 5001);
    assert_eq!(read_manifest(&dir, "bbb")["port"], 5002);
    assert_eq!(read_manifest(&dir, "ccc")["port"], 5003);
    assert_eq!(read_manifest(&dir, "bbb")["connectedDevices"], 1);
    assert_eq!(read_manifest(&dir, "ccc")["connectedDevices"], 2);
}
