//! Transcript watcher: the fallback source for sessions without hooks
//! (docs/session-detection.md#source-2-transcript-watcher-fallback, ADR-0010).
//! Watches `<config>/projects/**/*.jsonl`, reads only new lines and feeds
//! [`Source::Transcript`] events to the bridge driver.

mod parse;
mod tail;

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime};

use notify::{RecursiveMode, Watcher as _};

use crate::bridge::Input;
use crate::registry::{tool_summary, EventKind, Millis, RegistryEvent, Source};
use parse::{parse_line, LineKind};
use tail::Tail;

/// Files modified longer ago than this are not read at startup. Matches the
/// registry's transcript TTL, so we never resurrect an expired session.
const STARTUP_MAX_AGE: Duration = Duration::from_secs(30 * 60);
/// Changes are collected for this long before files are read, so a burst of
/// writes costs one read.
const DEBOUNCE: Duration = Duration::from_millis(150);

/// `<config>/projects`: `CLAUDE_CONFIG_DIR`, else `~/.claude` (Windows: `%USERPROFILE%\.claude`).
pub fn projects_dir() -> Option<PathBuf> {
    let non_empty = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    let config = match non_empty("CLAUDE_CONFIG_DIR") {
        Some(dir) => PathBuf::from(dir),
        None => {
            let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
            PathBuf::from(non_empty(home)?).join(".claude")
        }
    };
    Some(config.join("projects"))
}

/// A running watcher. Dropping it stops the file watch, which closes the
/// event channel and ends the thread.
pub struct TranscriptWatcher {
    _watcher: notify::RecommendedWatcher,
    _thread: JoinHandle<()>,
}

impl TranscriptWatcher {
    pub fn start(dir: &Path, inputs: Sender<Input>, clock: fn() -> Millis) -> io::Result<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                if let Ok(event) = event {
                    for path in event.paths {
                        let _ = tx.send(path);
                    }
                }
            })
            .map_err(io::Error::other)?;
        watcher
            .watch(dir, RecursiveMode::Recursive)
            .map_err(io::Error::other)?;

        let mut state = State::new(inputs, clock);
        for path in recent_transcripts(dir, STARTUP_MAX_AGE) {
            state.read(&path);
        }
        let thread = std::thread::Builder::new()
            .name("transcript-watcher".into())
            .spawn(move || run(&rx, &mut state))?;
        Ok(Self {
            _watcher: watcher,
            _thread: thread,
        })
    }
}

/// Blocks until something changes, then reads the changed files once the burst settles.
fn run(rx: &Receiver<PathBuf>, state: &mut State) {
    while let Ok(first) = rx.recv() {
        let mut dirty = HashSet::from([first]);
        loop {
            match rx.recv_timeout(DEBOUNCE) {
                Ok(path) => {
                    dirty.insert(path);
                }
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        }
        for path in dirty.into_iter().filter(|p| is_transcript(p)) {
            state.read(&path);
        }
    }
}

/// Per-file read positions and the last tool seen, plus the output channel.
struct State {
    inputs: Sender<Input>,
    clock: fn() -> Millis,
    files: HashMap<PathBuf, FileState>,
}

#[derive(Default)]
struct FileState {
    tail: Tail,
    last_tool: Option<String>,
}

impl State {
    fn new(inputs: Sender<Input>, clock: fn() -> Millis) -> Self {
        Self {
            inputs,
            clock,
            files: HashMap::new(),
        }
    }

    fn read(&mut self, path: &Path) {
        let Ok(metadata) = std::fs::metadata(path) else {
            // Deleted: forget it. The registry expires the session on its own.
            self.files.remove(path);
            return;
        };
        let file = self
            .files
            .entry(path.to_path_buf())
            .or_insert_with(|| FileState {
                tail: Tail::near_end(metadata.len()),
                last_tool: None,
            });
        let Ok(lines) = file.tail.read_new(path) else {
            return;
        };
        for line in lines.iter().filter_map(|l| parse_line(l)) {
            let kind = match line.kind {
                LineKind::Prompt => EventKind::PromptSubmitted,
                LineKind::ToolUse { tool, target } => {
                    file.last_tool = Some(tool.clone());
                    EventKind::ToolStarted {
                        summary: Some(tool_summary(&tool, target.as_deref())),
                        tool,
                    }
                }
                LineKind::ToolResult => match &file.last_tool {
                    Some(tool) => EventKind::ToolFinished { tool: tool.clone() },
                    None => continue,
                },
                LineKind::TurnEnded => EventKind::TurnEnded,
            };
            let _ = self.inputs.send(Input::Event(RegistryEvent {
                session_id: line.session_id,
                source: Source::Transcript,
                at: (self.clock)(),
                cwd: line.cwd,
                pid: None,
                kind,
            }));
        }
    }
}

/// A session transcript: `*.jsonl`, not inside a `subagents` folder.
fn is_transcript(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "jsonl")
        && !path.components().any(|c| c.as_os_str() == "subagents")
}

/// Session transcripts directly under `dir/<project>/` modified within `max_age`.
fn recent_transcripts(dir: &Path, max_age: Duration) -> Vec<PathBuf> {
    let now = SystemTime::now();
    let Ok(projects) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    projects
        .flatten()
        .filter_map(|project| std::fs::read_dir(project.path()).ok())
        .flat_map(|entries| entries.flatten())
        .map(|entry| entry.path())
        .filter(|path| is_transcript(path))
        .filter(|path| {
            std::fs::metadata(path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|modified| now.duration_since(modified).ok())
                .is_some_and(|age| age <= max_age)
        })
        .collect()
}

#[cfg(test)]
mod tests;
