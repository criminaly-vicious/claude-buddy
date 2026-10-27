//! Process liveness: tells the bridge when a session's `claude` process exits,
//! so buddies of closed terminals leave even without `SessionEnd`
//! (docs/session-detection.md#liveness).

use std::io;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(windows)]
mod windows;

/// Calls `on_exit` once, on a background thread, when process `pid` exits
/// (immediately if it is already gone). Errors mean the process cannot be
/// watched (unsupported OS, access denied); the session then only leaves on
/// `SessionEnd`.
pub fn watch(pid: u32, on_exit: impl FnOnce() + Send + 'static) -> io::Result<()> {
    #[cfg(windows)]
    return windows::watch(pid, on_exit);
    #[cfg(target_os = "linux")]
    return linux::watch(pid, linux::POLL_INTERVAL, on_exit);
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        let _ = (pid, on_exit);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "process liveness is not implemented on this OS",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::process::{Child, Command};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    fn long_running_child() -> Child {
        #[cfg(windows)]
        let child = Command::new("ping").args(["-n", "60", "127.0.0.1"]).spawn();
        #[cfg(not(windows))]
        let child = Command::new("sleep").arg("60").spawn();
        child.expect("spawn a child process")
    }

    fn watch_child(child: &Child) -> mpsc::Receiver<()> {
        let (tx, rx) = mpsc::channel();
        #[cfg(target_os = "linux")]
        linux::watch(child.id(), Duration::from_millis(50), move || {
            let _ = tx.send(());
        })
        .unwrap();
        #[cfg(not(target_os = "linux"))]
        watch(child.id(), move || {
            let _ = tx.send(());
        })
        .unwrap();
        rx
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn reports_exit_of_a_killed_process() {
        let mut child = long_running_child();
        let exited = watch_child(&child);
        assert!(
            exited.recv_timeout(Duration::from_millis(300)).is_err(),
            "still running"
        );

        child.kill().unwrap();
        child.wait().unwrap();
        exited
            .recv_timeout(Duration::from_secs(5))
            .expect("exit reported");
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn already_gone_process_is_reported_at_once() {
        let mut child = long_running_child();
        child.kill().unwrap();
        child.wait().unwrap();
        let exited = watch_child(&child);
        exited
            .recv_timeout(Duration::from_secs(5))
            .expect("exit reported");
    }
}
