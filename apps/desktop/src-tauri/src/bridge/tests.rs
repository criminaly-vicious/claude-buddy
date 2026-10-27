//! End-to-end tests over real loopback HTTP.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use super::*;
use crate::registry::{BuddyState, Config};

const SESSION: &str = "11111111-1111-4111-8111-111111111111";

struct Harness {
    bridge: Bridge,
    token: String,
    path: PathBuf,
    registry: Arc<Mutex<Registry>>,
    changes: Receiver<Change>,
}

fn start(name: &str) -> Harness {
    let config = Config {
        spawn_ms: 50,
        ..Config::default()
    };
    let dir =
        std::env::temp_dir().join(format!("claude-buddy-bridge-{name}-{}", std::process::id()));
    let path = dir.join("bridge.json");
    let registry = Arc::new(Mutex::new(Registry::new(config)));
    let (tx, changes) = mpsc::channel();
    let bridge = Bridge::start_at(&path, Arc::clone(&registry), move |c| {
        let _ = tx.send(c);
    })
    .unwrap();
    let file: BridgeFile = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(file.port, bridge.port());
    Harness {
        bridge,
        token: file.token,
        path,
        registry,
        changes,
    }
}

/// Sends a raw HTTP/1.1 request and returns the status code and body.
fn send(port: u16, request_head: &str, body: &[u8]) -> (u16, String) {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let head = format!(
        "{request_head}\r\nHost: 127.0.0.1\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    // The server may answer (e.g. 413) before reading the whole body.
    let _ = stream
        .write_all(head.as_bytes())
        .and_then(|_| stream.write_all(body));
    let mut response = String::new();
    let _ = stream.read_to_string(&mut response);
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| panic!("no status in {response:?}"));
    let body = response
        .split("\r\n\r\n")
        .nth(1)
        .unwrap_or_default()
        .to_string();
    (status, body)
}

fn post_event(h: &Harness, token: &str, json: &str) -> u16 {
    let head = format!(
        "POST /v1/events HTTP/1.1\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json"
    );
    send(h.bridge.port(), &head, json.as_bytes()).0
}

fn event_json(event: &str, extra: &str) -> String {
    format!(
        r#"{{"v":1,"event":"{event}","session_id":"{SESSION}","cwd":"C:\\dev\\demo","at":1,"claude_pid":9184{extra}}}"#
    )
}

fn next_change(h: &Harness) -> Change {
    h.changes
        .recv_timeout(Duration::from_secs(5))
        .expect("a registry change")
}

#[test]
fn writes_bridge_file_and_removes_it_on_drop() {
    let h = start("file");
    assert!(bridge_protocol::is_valid_token(&h.token));
    let path = h.path.clone();
    drop(h);
    assert!(!path.exists());
}

#[test]
fn valid_event_reaches_the_registry() {
    let h = start("valid");
    let status = post_event(
        &h,
        &h.token,
        &event_json("SessionStart", r#","start_source":"startup""#),
    );
    assert_eq!(status, 204);

    let Change::Update(view) = next_change(&h) else {
        panic!("expected an update");
    };
    assert_eq!(view.id, SESSION);
    assert_eq!(view.project, "demo");
    assert_eq!(view.pid, Some(9184));
    assert_eq!(view.state, BuddyState::Spawning);
    assert_eq!(h.registry.lock().unwrap().snapshot().len(), 1);
}

#[test]
fn driver_runs_registry_timers_without_input() {
    let h = start("timers");
    post_event(
        &h,
        &h.token,
        &event_json("SessionStart", r#","start_source":"startup""#),
    );
    next_change(&h); // spawning
                     // No further requests: the driver wakes at the spawn deadline on its own.
    let Change::Update(view) = next_change(&h) else {
        panic!("expected an update");
    };
    assert_eq!(view.state, BuddyState::Idle);
}

#[test]
fn tool_event_carries_a_summary() {
    let h = start("tool");
    let status = post_event(
        &h,
        &h.token,
        &event_json("PreToolUse", r#","tool_name":"Read","target":"index.ts""#),
    );
    assert_eq!(status, 204);
    let Change::Update(view) = next_change(&h) else {
        panic!("expected an update");
    };
    assert_eq!(view.summary.as_deref(), Some("Read · index.ts"));
}

#[test]
fn wrong_or_missing_token_is_401() {
    let h = start("auth");
    let wrong = "0".repeat(64);
    assert_eq!(post_event(&h, &wrong, &event_json("Stop", "")), 401);

    let head = "POST /v1/events HTTP/1.1\r\nContent-Type: application/json";
    assert_eq!(
        send(h.bridge.port(), head, event_json("Stop", "").as_bytes()).0,
        401
    );
    assert!(
        h.changes.try_recv().is_err(),
        "nothing reached the registry"
    );
}

#[test]
fn oversized_body_is_413() {
    let h = start("size");
    let big = event_json(
        "Stop",
        &format!(r#","padding":"{}""#, "x".repeat(17 * 1024)),
    );
    assert_eq!(post_event(&h, &h.token, &big), 413);
}

#[test]
fn bad_bodies_are_400() {
    let h = start("bad");
    assert_eq!(post_event(&h, &h.token, "not json"), 400);
    // Missing tool_name on a tool event.
    assert_eq!(post_event(&h, &h.token, &event_json("PreToolUse", "")), 400);
    // Field over its limit.
    let long = format!(r#","tool_name":"Read","target":"{}""#, "t".repeat(61));
    assert_eq!(
        post_event(&h, &h.token, &event_json("PreToolUse", &long)),
        400
    );
    // Unsupported version.
    let v2 = event_json("Stop", "").replace(r#""v":1"#, r#""v":2"#);
    assert_eq!(post_event(&h, &h.token, &v2), 400);
}

#[test]
fn unknown_event_is_202_and_ignored() {
    let h = start("unknown");
    assert_eq!(
        post_event(&h, &h.token, &event_json("SomethingNew", "")),
        202
    );
    assert!(h.changes.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn ignored_known_event_is_204() {
    let h = start("ignored");
    assert_eq!(
        post_event(&h, &h.token, &event_json("SubagentStart", "")),
        204
    );
    assert!(h.changes.recv_timeout(Duration::from_millis(200)).is_err());
}

#[test]
fn wrong_content_type_is_415() {
    let h = start("type");
    let head = format!(
        "POST /v1/events HTTP/1.1\r\nAuthorization: Bearer {}\r\nContent-Type: text/plain",
        h.token
    );
    assert_eq!(
        send(h.bridge.port(), &head, event_json("Stop", "").as_bytes()).0,
        415
    );
}

#[test]
fn other_paths_and_methods() {
    let h = start("routes");
    let auth = format!("Authorization: Bearer {}", h.token);
    assert_eq!(
        send(
            h.bridge.port(),
            &format!("GET /v1/events HTTP/1.1\r\n{auth}"),
            b""
        )
        .0,
        405
    );
    assert_eq!(
        send(
            h.bridge.port(),
            &format!("GET /nope HTTP/1.1\r\n{auth}"),
            b""
        )
        .0,
        404
    );
}

#[test]
fn health_needs_the_token() {
    let h = start("health");
    let (status, body) = send(
        h.bridge.port(),
        &format!(
            "GET /v1/health HTTP/1.1\r\nAuthorization: Bearer {}",
            h.token
        ),
        b"",
    );
    assert_eq!((status, body.as_str()), (200, r#"{"protocol":1}"#));
    assert_eq!(send(h.bridge.port(), "GET /v1/health HTTP/1.1", b"").0, 401);
}

#[test]
fn constant_time_eq_compares_exactly() {
    use super::server::constant_time_eq;
    assert!(constant_time_eq(b"abc", b"abc"));
    assert!(!constant_time_eq(b"abc", b"abd"));
    assert!(!constant_time_eq(b"abc", b"ab"));
}

#[test]
fn stalled_client_does_not_block_other_requests() {
    let h = start("stall");
    // Authenticated client that promises a body and never sends it. Over
    // 1 KiB, because tiny_http buffers smaller bodies on its own thread.
    let mut stalled = TcpStream::connect(("127.0.0.1", h.bridge.port())).unwrap();
    let head = format!(
        "POST /v1/events HTTP/1.1\r\nHost: 127.0.0.1\r\nAuthorization: Bearer {}\r\nContent-Type: application/json\r\nContent-Length: 10000\r\n\r\n{{",
        h.token
    );
    stalled.write_all(head.as_bytes()).unwrap();
    std::thread::sleep(Duration::from_millis(100));

    let status = post_event(
        &h,
        &h.token,
        &event_json("SessionStart", r#","start_source":"startup""#),
    );
    assert_eq!(status, 204);
    next_change(&h);
    drop(stalled);
}

/// Runs the driver directly with a fake process watcher that reports every
/// process as exited right away.
#[test]
fn driver_expires_session_when_its_process_exits() {
    fn exits_at_once(_pid: u32, on_exit: Box<dyn FnOnce() + Send>) -> io::Result<()> {
        on_exit();
        Ok(())
    }
    let registry = Mutex::new(Registry::new(Config {
        spawn_ms: 10,
        min_state_ms: 10,
        exit_grace_ms: 50,
        leave_ms: 50,
        ..Config::default()
    }));
    let (inputs, rx) = mpsc::channel();
    let (changes_tx, changes) = mpsc::channel();
    inputs
        .send(Input::Event(RegistryEvent {
            session_id: SESSION.into(),
            source: crate::registry::Source::Hooks,
            at: now(),
            cwd: Some("/demo".into()),
            pid: Some(4242),
            kind: crate::registry::EventKind::SessionStarted,
        }))
        .unwrap();

    let thread = std::thread::spawn({
        let inputs = inputs.clone();
        move || {
            drive(&registry, &rx, &inputs, now, exits_at_once, |c| {
                let _ = changes_tx.send(c);
            })
        }
    });

    let mut saw_leaving = false;
    loop {
        match changes
            .recv_timeout(Duration::from_secs(5))
            .expect("a change")
        {
            Change::Update(view) if view.state == BuddyState::Leaving => saw_leaving = true,
            Change::Remove(id) => {
                assert_eq!(id, SESSION);
                break;
            }
            Change::Update(_) => {}
        }
    }
    assert!(saw_leaving);
    inputs.send(Input::Shutdown).unwrap();
    thread.join().unwrap();
}
