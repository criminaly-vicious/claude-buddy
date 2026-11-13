//! "Is the bridge reachable the way `buddy-hook` reaches it?" after the
//! tray installs hooks: read `bridge.json`, then `GET /v1/health` with its
//! token on loopback. A tiny blocking client, like `buddy-hook`'s.

use std::fmt;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::path::Path;
use std::time::Duration;

use bridge_protocol::{BridgeFile, HEALTH_PATH};

const TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, PartialEq, Eq)]
pub enum HealthError {
    /// `bridge.json` is missing or unreadable: the bridge is not running.
    NoBridgeFile,
    /// `bridge.json` is from another protocol version or malformed.
    UnusableBridgeFile,
    /// Nothing answered on the port, or the connection failed.
    Unreachable(String),
    /// It answered, but not with `200`.
    Status(u16),
}

impl fmt::Display for HealthError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoBridgeFile => write!(f, "the bridge file is missing"),
            Self::UnusableBridgeFile => write!(f, "the bridge file is not usable"),
            Self::Unreachable(error) => write!(f, "the bridge did not answer ({error})"),
            Self::Status(status) => write!(f, "the bridge answered with status {status}"),
        }
    }
}

/// Checks the bridge described by the `bridge.json` at `path`.
pub fn check(path: &Path) -> Result<(), HealthError> {
    let bytes = std::fs::read(path).map_err(|_| HealthError::NoBridgeFile)?;
    let file: BridgeFile =
        serde_json::from_slice(&bytes).map_err(|_| HealthError::UnusableBridgeFile)?;
    if !file.is_usable() {
        return Err(HealthError::UnusableBridgeFile);
    }
    let status =
        get(file.port, &file.token).map_err(|e| HealthError::Unreachable(e.to_string()))?;
    if status == 200 {
        Ok(())
    } else {
        Err(HealthError::Status(status))
    }
}

/// `GET /v1/health` on `127.0.0.1:port`; returns the status code.
fn get(port: u16, token: &str) -> std::io::Result<u16> {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, TIMEOUT)?;
    stream.set_read_timeout(Some(TIMEOUT))?;
    stream.set_write_timeout(Some(TIMEOUT))?;
    write!(
        stream,
        "GET {HEALTH_PATH} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n"
    )?;
    // The status line is all we need; it fits in the first read.
    let mut head = [0u8; 64];
    let n = stream.read(&mut head)?;
    parse_status(&head[..n]).ok_or_else(|| std::io::Error::other("not an HTTP response"))
}

/// The status code of an `HTTP/1.x NNN ...` status line.
fn parse_status(head: &[u8]) -> Option<u16> {
    let line = std::str::from_utf8(head).ok()?;
    let mut parts = line.split(' ');
    parts.next().filter(|v| v.starts_with("HTTP/1."))?;
    parts.next()?.get(..3)?.parse().ok()
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::bridge::Bridge;
    use crate::registry::Registry;

    fn temp(name: &str) -> std::path::PathBuf {
        std::env::temp_dir()
            .join(format!("claude-buddy-health-{name}-{}", std::process::id()))
            .join("bridge.json")
    }

    #[test]
    fn a_running_bridge_is_healthy() {
        let path = temp("ok");
        let _bridge = Bridge::start_at(&path, Arc::new(Mutex::new(Registry::default())), |_| {})
            .expect("bridge starts");
        assert_eq!(check(&path), Ok(()));
    }

    #[test]
    fn missing_or_bad_bridge_file() {
        let path = temp("missing");
        assert_eq!(check(&path), Err(HealthError::NoBridgeFile));
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"{\"protocol\":1}").unwrap();
        assert_eq!(check(&path), Err(HealthError::UnusableBridgeFile));
        std::fs::write(
            &path,
            format!(
                r#"{{"protocol":99,"port":1,"token":"{}","pid":1,"started_at":0}}"#,
                "a".repeat(64)
            ),
        )
        .unwrap();
        assert_eq!(check(&path), Err(HealthError::UnusableBridgeFile));
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn wrong_token_is_reported_as_status() {
        let path = temp("token");
        let _bridge = Bridge::start_at(&path, Arc::new(Mutex::new(Registry::default())), |_| {})
            .expect("bridge starts");
        let mut file: BridgeFile = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        file.token = "b".repeat(64);
        let forged = temp("token-forged");
        std::fs::create_dir_all(forged.parent().unwrap()).unwrap();
        std::fs::write(&forged, serde_json::to_vec(&file).unwrap()).unwrap();
        assert_eq!(check(&forged), Err(HealthError::Status(401)));
        std::fs::remove_file(&forged).unwrap();
    }

    #[test]
    fn nothing_listening_is_unreachable() {
        // Bind then drop to find a closed port.
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        assert!(get(port, &"a".repeat(64)).is_err());
    }

    #[test]
    fn parses_status_lines() {
        assert_eq!(parse_status(b"HTTP/1.1 200 OK\r\n"), Some(200));
        assert_eq!(parse_status(b"HTTP/1.0 401 Unauthorized"), Some(401));
        assert_eq!(parse_status(b"SSH-2.0"), None);
        assert_eq!(parse_status(b""), None);
    }
}
