//! Linux (development only): checks `/proc/<pid>/stat` periodically. The
//! process start time is compared too, so a reused PID counts as an exit.

use std::io;
use std::time::Duration;

pub const POLL_INTERVAL: Duration = Duration::from_secs(5);

pub fn watch(
    pid: u32,
    interval: Duration,
    on_exit: impl FnOnce() + Send + 'static,
) -> io::Result<()> {
    let started = start_time(pid);
    std::thread::Builder::new()
        .name(format!("liveness-{pid}"))
        .spawn(move || {
            if let Some(started) = started {
                while start_time(pid) == Some(started) {
                    std::thread::sleep(interval);
                }
            }
            on_exit();
        })?;
    Ok(())
}

/// Start time of `pid` in clock ticks since boot (field 22 of `/proc/<pid>/stat`),
/// or `None` if the process does not exist or is a zombie.
fn start_time(pid: u32) -> Option<u64> {
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    // The command name (field 2) is in parentheses and may contain spaces.
    let rest = &stat[stat.rfind(')')? + 1..];
    let mut fields = rest.split_whitespace();
    // `rest` starts at field 3 (state); a zombie has already exited.
    if fields.next()? == "Z" {
        return None;
    }
    fields.nth(22 - 4)?.parse().ok()
}
