//! `buddy-hook`: forwards one Claude Code hook event to the Claude Buddy
//! bridge (docs/architecture.md#bridge-protocol). It must never block or fail
//! a session: the caller (`main`) always exits 0 and prints nothing, and
//! every step here gives up quietly by returning `None` or an error.

use std::io::{self, Write};
use std::net::{Shutdown, SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use bridge_protocol::{
    target, BridgeFile, EventBody, EVENTS_PATH, MAX_BODY_BYTES, PROTOCOL_VERSION,
};
use serde_json::Value;

/// Total time budget for connect + write (docs: 300 ms).
pub const TIMEOUT: Duration = Duration::from_millis(300);

/// Builds the wire event from a hook's stdin payload. `event_arg` is the
/// event name the installer passes as an argument; the payload's own
/// `hook_event_name` wins when present. Returns `None` for anything that
/// cannot be sent (bad JSON, missing fields, over-limit values).
///
/// Only the fields in the protocol are copied. Prompts, tool inputs and
/// outputs, messages and transcript paths never leave this function;
/// `tool_input` is used only to derive the short `target`.
pub fn build_event(
    event_arg: Option<&str>,
    stdin: &str,
    claude_pid: Option<&str>,
    now_ms: u64,
) -> Option<EventBody> {
    let payload: Value = serde_json::from_str(stdin).ok()?;
    let text = |name: &str| {
        payload
            .get(name)
            .and_then(Value::as_str)
            .map(str::to_string)
    };
    let event = text("hook_event_name").or_else(|| event_arg.map(str::to_string))?;
    let tool_name = text("tool_name");
    // A target only where the buddy shows it: the tool that is starting.
    let derived = match (event.as_str(), &tool_name) {
        ("PreToolUse", Some(tool)) => {
            target::derive(tool, payload.get("tool_input").unwrap_or(&Value::Null))
        }
        _ => None,
    };

    let mut body = EventBody {
        v: PROTOCOL_VERSION,
        session_id: text("session_id")?,
        cwd: text("cwd")?,
        at: now_ms,
        claude_pid: claude_pid.and_then(|pid| pid.trim().parse().ok()),
        tool_name,
        target: derived,
        notification_type: (event == "Notification")
            .then(|| text("notification_type"))
            .flatten(),
        start_source: (event == "SessionStart").then(|| text("source")).flatten(),
        agent_id: text("agent_id"),
        event,
    };
    body.clamp_lengths();
    body.validate().ok()?;
    Some(body)
}

/// Reads `bridge.json` and returns where to send and the token, if the app
/// is (probably) running with a protocol we speak.
pub fn read_bridge_file(contents: &str) -> Option<(SocketAddr, String)> {
    let file: BridgeFile = serde_json::from_str(contents).ok()?;
    if !file.is_usable() {
        return None;
    }
    Some((SocketAddr::from(([127, 0, 0, 1], file.port)), file.token))
}

/// Sends `body` with one HTTP/1.1 request and returns without reading the
/// response. Connect and write share the deadline.
pub fn send(addr: SocketAddr, token: &str, body: &EventBody, deadline: Instant) -> io::Result<()> {
    let json = serde_json::to_vec(body).map_err(io::Error::other)?;
    if json.len() > MAX_BODY_BYTES {
        return Err(io::Error::other("event too large"));
    }
    let remaining = || {
        deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "out of time"))
    };
    let mut stream = TcpStream::connect_timeout(&addr, remaining()?)?;
    stream.set_nodelay(true)?;
    stream.set_write_timeout(Some(remaining()?))?;
    let head = format!(
        "POST {EVENTS_PATH} HTTP/1.1\r\nHost: {addr}\r\nAuthorization: Bearer {token}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        json.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&json)?;
    stream.flush()?;
    // Done sending; the bridge's answer is not needed.
    let _ = stream.shutdown(Shutdown::Write);
    Ok(())
}

#[cfg(test)]
mod tests;
