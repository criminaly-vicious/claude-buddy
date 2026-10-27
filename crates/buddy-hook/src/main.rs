//! Entry point. Whatever happens (no app running, bad input, a bug that
//! panics), this exits 0 and writes nothing, because Claude Code shows any
//! hook output or failure to the user.

use std::io::Read;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn main() {
    let started = Instant::now();
    // No panic message on stderr; `catch_unwind` below keeps the exit code at 0.
    std::panic::set_hook(Box::new(|_| {}));
    let _ = std::panic::catch_unwind(|| forward(started));
    std::process::exit(0);
}

fn forward(started: Instant) -> Option<()> {
    let event_arg = std::env::args().nth(1);
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin).ok()?;
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    let pid = std::env::var("CLAUDE_PID").ok();
    let body = buddy_hook::build_event(event_arg.as_deref(), &stdin, pid.as_deref(), now_ms)?;

    let path = bridge_protocol::bridge_file_path()?;
    let contents = std::fs::read_to_string(path).ok()?;
    let (addr, token) = buddy_hook::read_bridge_file(&contents)?;
    buddy_hook::send(addr, &token, &body, started + buddy_hook::TIMEOUT).ok()
}
