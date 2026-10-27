//! Runs the real `buddy-hook` binary against a fake bridge.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

const TOKEN: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../tests/fixtures/hooks/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(path).unwrap()
}

/// A `bridge.json` pointing at `port`, in a fresh temp folder.
fn bridge_file(name: &str, port: u16) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("buddy-hook-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bridge.json");
    std::fs::write(
        &path,
        format!(r#"{{"protocol":1,"port":{port},"token":"{TOKEN}","pid":1,"started_at":0}}"#),
    )
    .unwrap();
    path
}

/// Runs the hook with `stdin`, returning its output and how long it took.
fn run(bridge: &PathBuf, event: &str, stdin: &str) -> (Output, Duration) {
    let started = Instant::now();
    let mut child = Command::new(env!("CARGO_BIN_EXE_buddy-hook"))
        .arg(event)
        .env("CLAUDE_BUDDY_BRIDGE_FILE", bridge)
        .env("CLAUDE_PID", "4242")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    (output, started.elapsed())
}

fn assert_silent_success(output: &Output) {
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stdout.is_empty(), "stdout: {:?}", output.stdout);
    assert!(
        output.stderr.is_empty(),
        "stderr: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Accepts one connection and returns everything the client sent.
fn fake_bridge() -> (u16, mpsc::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut request = String::new();
        let _ = stream.read_to_string(&mut request);
        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\n\r\n");
        let _ = tx.send(request);
    });
    (port, rx)
}

#[test]
fn forwards_a_sanitised_event_with_the_token() {
    let (port, received) = fake_bridge();
    let bridge = bridge_file("forward", port);
    let (output, _) = run(
        &bridge,
        "PreToolUse",
        &fixture("captured/pre-tool-use-read.json"),
    );
    assert_silent_success(&output);

    let request = received.recv_timeout(Duration::from_secs(5)).unwrap();
    let (head, body) = request.split_once("\r\n\r\n").unwrap();
    assert!(head.starts_with("POST /v1/events HTTP/1.1\r\n"), "{head}");
    assert!(head.contains(&format!("Authorization: Bearer {TOKEN}")));
    assert!(head.contains("Content-Type: application/json"));
    assert!(head.contains(&format!("Content-Length: {}", body.len())));

    let mut json: serde_json::Value = serde_json::from_str(body).unwrap();
    assert!(json["at"].as_u64().unwrap() > 0);
    json.as_object_mut().unwrap().remove("at");
    assert_eq!(
        json,
        serde_json::json!({
            "v": 1,
            "event": "PreToolUse",
            "session_id": "11111111-1111-4111-8111-111111111111",
            "cwd": "/home/user/projects/demo",
            "claude_pid": 4242,
            "tool_name": "Read",
            "target": "notes.txt"
        })
    );
}

#[test]
fn prompt_text_never_reaches_the_bridge() {
    let (port, received) = fake_bridge();
    let bridge = bridge_file("prompt", port);
    let stdin = fixture("captured/user-prompt-submit.json");
    let prompt: serde_json::Value = serde_json::from_str(&stdin).unwrap();
    let (output, _) = run(&bridge, "UserPromptSubmit", &stdin);
    assert_silent_success(&output);
    let request = received.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(!request.contains(prompt["prompt"].as_str().unwrap()));
    assert!(!request.contains("\"prompt\""));
}

#[test]
fn no_bridge_file_exits_quietly() {
    let missing = std::env::temp_dir().join("buddy-hook-no-such-dir/bridge.json");
    let (output, elapsed) = run(&missing, "Stop", &fixture("captured/stop.json"));
    assert_silent_success(&output);
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn closed_port_exits_quietly() {
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    // The listener is dropped: nothing listens on `port` any more.
    let bridge = bridge_file("closed", port);
    let (output, elapsed) = run(&bridge, "Stop", &fixture("captured/stop.json"));
    assert_silent_success(&output);
    assert!(elapsed < Duration::from_secs(2));
}

#[test]
fn a_bridge_that_never_answers_does_not_hold_the_hook() {
    // Listening but never accepting or reading: the hook must not wait for it.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let bridge = bridge_file("silent", listener.local_addr().unwrap().port());
    let (output, elapsed) = run(&bridge, "Stop", &fixture("captured/stop.json"));
    assert_silent_success(&output);
    // 300 ms budget plus process start-up.
    assert!(elapsed < Duration::from_millis(800), "took {elapsed:?}");
    drop(listener);
}

#[test]
fn invalid_stdin_exits_quietly() {
    let (port, received) = fake_bridge();
    let bridge = bridge_file("invalid", port);
    for stdin in ["", "{not json", "\u{0}\u{1}binary"] {
        let (output, _) = run(&bridge, "Stop", stdin);
        assert_silent_success(&output);
    }
    assert!(
        received.recv_timeout(Duration::from_millis(300)).is_err(),
        "nothing sent"
    );
}

#[test]
fn no_arguments_and_no_stdin_exit_quietly() {
    let output = Command::new(env!("CARGO_BIN_EXE_buddy-hook"))
        .env("CLAUDE_BUDDY_BRIDGE_FILE", "/nonexistent/bridge.json")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert_silent_success(&output);
}
