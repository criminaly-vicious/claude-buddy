//! Local bridge: receives hook events from `buddy-hook` over loopback HTTP
//! and feeds them to the session registry. Protocol: `crates/bridge-protocol`
//! and docs/architecture.md#bridge-protocol. Decisions: ADR-0003, ADR-0009.

mod file;
mod normalize;
mod server;
#[cfg(windows)]
mod win_acl;

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bridge_protocol::{BridgeFile, PROTOCOL_VERSION};

use crate::registry::{Change, Millis, Registry, RegistryEvent};

/// Messages to the driver thread, which owns all registry changes.
pub enum Input {
    Event(RegistryEvent),
    /// A session's `claude` process exited (from [`crate::liveness`]).
    ProcessExited {
        session_id: String,
        pid: u32,
    },
    Shutdown,
}

/// Starts watching a process; calls the callback when it exits.
type WatchFn = fn(u32, Box<dyn FnOnce() + Send>) -> io::Result<()>;

fn watch_process(pid: u32, on_exit: Box<dyn FnOnce() + Send>) -> io::Result<()> {
    crate::liveness::watch(pid, on_exit)
}

/// A running bridge. Dropping it stops the server and the driver and deletes `bridge.json`.
pub struct Bridge {
    // Field order is drop order: stop accepting requests, then the driver, then the file.
    server: server::Server,
    _driver: Driver,
    _file: file::FileGuard,
}

impl Bridge {
    /// Starts the bridge with `bridge.json` at the protocol's standard path.
    pub fn start(
        registry: Arc<Mutex<Registry>>,
        emit: impl FnMut(Change) + Send + 'static,
    ) -> io::Result<Self> {
        let path = bridge_protocol::bridge_file_path()
            .ok_or_else(|| io::Error::other("cannot locate the bridge file folder"))?;
        Self::start_at(&path, registry, emit)
    }

    /// Like [`Bridge::start`] with an explicit `bridge.json` path.
    pub fn start_at(
        path: &Path,
        registry: Arc<Mutex<Registry>>,
        emit: impl FnMut(Change) + Send + 'static,
    ) -> io::Result<Self> {
        let token = file::new_token()?;
        let (inputs, rx) = mpsc::channel();
        let driver = Driver::spawn(registry, rx, inputs.clone(), emit)?;
        let server = server::Server::start(token.clone(), inputs, now)?;
        let contents = BridgeFile {
            protocol: PROTOCOL_VERSION,
            port: server.port,
            token,
            pid: std::process::id(),
            started_at: now(),
        };
        let file = file::write(path, &contents)?;
        Ok(Self {
            server,
            _driver: driver,
            _file: file,
        })
    }

    pub fn port(&self) -> u16 {
        self.server.port
    }
}

struct Driver {
    inputs: Sender<Input>,
    thread: Option<JoinHandle<()>>,
}

impl Driver {
    fn spawn(
        registry: Arc<Mutex<Registry>>,
        rx: Receiver<Input>,
        inputs: Sender<Input>,
        emit: impl FnMut(Change) + Send + 'static,
    ) -> io::Result<Self> {
        let thread = std::thread::Builder::new()
            .name("bridge-driver".into())
            .spawn({
                let inputs = inputs.clone();
                move || drive(&registry, &rx, &inputs, now, watch_process, emit)
            })?;
        Ok(Self {
            inputs,
            thread: Some(thread),
        })
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        let _ = self.inputs.send(Input::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Applies inputs and runs registry timers. Sleeps until the next input or
/// the registry's next deadline, so it never polls.
fn drive(
    registry: &Mutex<Registry>,
    rx: &Receiver<Input>,
    inputs: &Sender<Input>,
    clock: fn() -> Millis,
    watch: WatchFn,
    mut emit: impl FnMut(Change),
) {
    // (session, pid) pairs already being watched.
    let mut watched: HashSet<(String, u32)> = HashSet::new();
    loop {
        let deadline = lock(registry).next_deadline();
        let input = match deadline {
            Some(at) => {
                let wait = Duration::from_millis(at.saturating_sub(clock()));
                match rx.recv_timeout(wait) {
                    Ok(input) => Some(input),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
            None => match rx.recv() {
                Ok(input) => Some(input),
                Err(_) => return,
            },
        };

        let changes = {
            let mut registry = lock(registry);
            let mut changes = match input {
                Some(Input::Event(event)) => {
                    if let Some(pid) = event.pid {
                        let key = (event.session_id.clone(), pid);
                        if !watched.contains(&key) {
                            let inputs = inputs.clone();
                            let session_id = key.0.clone();
                            let on_exit = Box::new(move || {
                                let _ = inputs.send(Input::ProcessExited { session_id, pid });
                            });
                            // A process we cannot watch only leaves on SessionEnd.
                            if watch(pid, on_exit).is_ok() {
                                watched.insert(key);
                            }
                        }
                    }
                    registry.apply(event)
                }
                Some(Input::ProcessExited { session_id, pid }) => {
                    registry.process_exited(&session_id, pid, clock());
                    Vec::new()
                }
                Some(Input::Shutdown) => return,
                None => Vec::new(),
            };
            changes.extend(registry.tick(clock()));
            changes
        };
        for change in &changes {
            if let Change::Remove(id) = change {
                watched.retain(|(session_id, _)| session_id != id);
            }
        }
        changes.into_iter().for_each(&mut emit);
    }
}

/// The registry stays usable even if a holder panicked.
fn lock(registry: &Mutex<Registry>) -> std::sync::MutexGuard<'_, Registry> {
    registry
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn now() -> Millis {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as Millis)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
