//! `POST /v1/events` and `GET /v1/health` on 127.0.0.1 (docs/architecture.md#endpoint-post-v1events).

use std::io::Read;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::thread::JoinHandle;

use bridge_protocol::{EventBody, EVENTS_PATH, HEALTH_PATH, MAX_BODY_BYTES, PROTOCOL_VERSION};
use tiny_http::{Header, Method, Request, Response, StatusCode};

use super::normalize::to_registry_event;
use super::Input;
use crate::registry::Millis;

pub struct Server {
    http: Arc<tiny_http::Server>,
    thread: Option<JoinHandle<()>>,
    pub port: u16,
}

impl Server {
    /// Binds a random free loopback port and serves requests on a new thread.
    pub fn start(
        token: String,
        inputs: Sender<Input>,
        clock: fn() -> Millis,
    ) -> std::io::Result<Self> {
        let http = tiny_http::Server::http("127.0.0.1:0").map_err(std::io::Error::other)?;
        let port = http
            .server_addr()
            .to_ip()
            .map(|addr| addr.port())
            .ok_or_else(|| std::io::Error::other("bridge is not bound to an IP address"))?;
        let http = Arc::new(http);
        let worker = Arc::clone(&http);
        let thread = std::thread::Builder::new()
            .name("bridge-http".into())
            .spawn(move || {
                for mut request in worker.incoming_requests() {
                    let (status, body) = handle(&mut request, &token, &inputs, clock);
                    let _ = request.respond(response(status, body));
                }
            })?;
        Ok(Self {
            http,
            thread: Some(thread),
            port,
        })
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.http.unblock();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn response(status: u16, body: Option<String>) -> Response<std::io::Cursor<Vec<u8>>> {
    let response = Response::from_data(body.unwrap_or_default().into_bytes())
        .with_status_code(StatusCode(status));
    if status == 200 {
        let json = Header::from_bytes("Content-Type", "application/json").expect("static header");
        response.with_header(json)
    } else {
        response
    }
}

/// Returns the status code and an optional JSON body.
fn handle(
    request: &mut Request,
    token: &str,
    inputs: &Sender<Input>,
    clock: fn() -> Millis,
) -> (u16, Option<String>) {
    let path = request.url().split('?').next().unwrap_or_default();
    let route = match (path, request.method()) {
        (EVENTS_PATH, Method::Post) => Route::Events,
        (HEALTH_PATH, Method::Get) => Route::Health,
        (EVENTS_PATH | HEALTH_PATH, _) => return (405, None),
        _ => return (404, None),
    };
    if !authorized(request, token) {
        return (401, None);
    }
    if let Route::Health = route {
        return (200, Some(format!(r#"{{"protocol":{PROTOCOL_VERSION}}}"#)));
    }

    if !header_is(request, "Content-Type", |v| {
        v.split(';')
            .next()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case("application/json")
    }) {
        return (415, None);
    }
    if request
        .body_length()
        .is_some_and(|len| len > MAX_BODY_BYTES)
    {
        return (413, None);
    }
    let mut raw = Vec::new();
    let limit = MAX_BODY_BYTES as u64 + 1;
    if request
        .as_reader()
        .take(limit)
        .read_to_end(&mut raw)
        .is_err()
    {
        return (400, None);
    }
    if raw.len() > MAX_BODY_BYTES {
        return (413, None);
    }

    let Ok(body) = serde_json::from_slice::<EventBody>(&raw) else {
        return (400, None);
    };
    if body.validate().is_err() {
        return (400, None);
    }
    if !body.is_known_event() {
        return (202, None);
    }
    if let Some(event) = to_registry_event(body, clock()) {
        let _ = inputs.send(Input::Event(event));
    }
    (204, None)
}

enum Route {
    Events,
    Health,
}

fn authorized(request: &Request, token: &str) -> bool {
    let mut ok = false;
    for header in request.headers() {
        if header.field.equiv("Authorization") {
            let value = header.value.as_str();
            if let Some(given) = value.strip_prefix("Bearer ") {
                ok = constant_time_eq(given.as_bytes(), token.as_bytes());
            }
        }
    }
    ok
}

fn header_is(request: &Request, name: &'static str, check: impl Fn(&str) -> bool) -> bool {
    request
        .headers()
        .iter()
        .find(|h| h.field.equiv(name))
        .is_some_and(|h| check(h.value.as_str()))
}

/// Compares without an early exit, so timing does not reveal the token.
pub(super) fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
