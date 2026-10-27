//! Session registry: the single source of truth for every Claude Code session
//! and the state its buddy shows. Pure logic with no I/O: callers pass the
//! current time in, so every transition is deterministic and testable.
//!
//! Inputs are normalised [`RegistryEvent`]s (adapted from hooks by the bridge
//! and from transcripts by the watcher). Outputs are [`Change`]s, which the app
//! forwards to the overlay as `sessions:update` / `sessions:remove` events.
//! See docs/session-detection.md#state-machine.

mod tool_map;

use std::collections::HashMap;

use serde::Serialize;

pub use tool_map::state_for_tool;

/// Milliseconds since the Unix epoch.
pub type Millis = u64;

/// Tauri event carrying the full `Vec<SessionView>` when the overlay starts.
pub const EVENT_SNAPSHOT: &str = "sessions:snapshot";
/// Tauri event carrying one changed `SessionView`.
pub const EVENT_UPDATE: &str = "sessions:update";
/// Tauri event carrying the id of a removed session.
pub const EVENT_REMOVE: &str = "sessions:remove";

const SUMMARY_MAX_CHARS: usize = 80;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BuddyState {
    Spawning,
    Idle,
    Working,
    Researching,
    Waiting,
    Sleeping,
    Leaving,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Source {
    Hooks,
    Transcript,
}

/// What happened in a session, independent of where we learned it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventKind {
    SessionStarted,
    PromptSubmitted,
    /// `summary` must already be sanitised by the source (tool name + short target).
    ToolStarted {
        tool: String,
        summary: Option<String>,
    },
    ToolFinished {
        tool: String,
    },
    /// Permission prompt, question or idle prompt: the user is needed.
    NeedsInput,
    TurnEnded,
    SessionEnded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEvent {
    pub session_id: String,
    pub source: Source,
    pub at: Millis,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
    pub kind: EventKind,
}

/// What the overlay sees. Mirrors `SessionView` in apps/desktop/src/types.ts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionView {
    pub id: String,
    pub project: String,
    pub cwd: String,
    pub state: BuddyState,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub since: Millis,
    pub source: Source,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Update(SessionView),
    Remove(String),
}

#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Minimum time a state stays on screen before a calmer one replaces it.
    pub min_state_ms: Millis,
    /// Length of the spawn animation.
    pub spawn_ms: Millis,
    /// Idle time without events before the buddy falls asleep.
    pub idle_timeout_ms: Millis,
    /// Length of the leave animation before the session is removed.
    pub leave_ms: Millis,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_state_ms: 600,
            spawn_ms: 1_000,
            idle_timeout_ms: 5 * 60 * 1_000,
            leave_ms: 2_000,
        }
    }
}

#[derive(Debug)]
struct Session {
    view: SessionView,
    /// State to show once the current one has been visible long enough.
    pending: Option<BuddyState>,
    last_event_at: Millis,
    created_at: Millis,
}

#[derive(Debug, Default)]
pub struct Registry {
    config: Config,
    sessions: HashMap<String, Session>,
}

impl Registry {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
        }
    }

    /// All live sessions, oldest first (stable slot order for the overlay).
    pub fn snapshot(&self) -> Vec<SessionView> {
        let mut sessions: Vec<&Session> = self.sessions.values().collect();
        sessions.sort_by(|a, b| (a.created_at, &a.view.id).cmp(&(b.created_at, &b.view.id)));
        sessions.into_iter().map(|s| s.view.clone()).collect()
    }

    /// Applies one event and returns what changed.
    pub fn apply(&mut self, event: RegistryEvent) -> Vec<Change> {
        let config = self.config;
        let Some(session) = self.sessions.get_mut(&event.session_id) else {
            return self.create(event);
        };

        // Hooks are exact; once a session has them, transcript inference is noise.
        if event.source == Source::Transcript && session.view.source == Source::Hooks {
            return Vec::new();
        }
        if session.view.state == BuddyState::Leaving {
            return Vec::new();
        }

        let mut changed = false;
        if event.source == Source::Hooks && session.view.source == Source::Transcript {
            session.view.source = Source::Hooks;
            changed = true;
        }
        changed |= session.absorb_metadata(&event);
        session.last_event_at = event.at;

        let target = session.target_state(&event.kind);
        if let Some(target) = target {
            changed |= session.request_state(target, event.at, &config);
        }
        if changed {
            vec![Change::Update(session.view.clone())]
        } else {
            Vec::new()
        }
    }

    /// Advances timers (pending states, spawn end, sleep, removal).
    pub fn tick(&mut self, now: Millis) -> Vec<Change> {
        let config = self.config;
        let mut changes = Vec::new();
        let mut removed = Vec::new();

        for (id, session) in &mut self.sessions {
            let elapsed = now.saturating_sub(session.view.since);
            if session.view.state == BuddyState::Leaving {
                if elapsed >= config.leave_ms {
                    removed.push(id.clone());
                }
                continue;
            }

            let next = if let Some(pending) = session.pending {
                (elapsed >= config.min_for(session.view.state)).then_some(pending)
            } else if session.view.state == BuddyState::Spawning {
                (elapsed >= config.spawn_ms).then_some(BuddyState::Idle)
            } else if session.view.state == BuddyState::Idle {
                let quiet = now.saturating_sub(session.last_event_at);
                (quiet >= config.idle_timeout_ms).then_some(BuddyState::Sleeping)
            } else {
                None
            };

            if let Some(next) = next {
                if session.request_state(next, now, &config) {
                    changes.push(Change::Update(session.view.clone()));
                }
            }
        }

        removed.sort();
        for id in removed {
            self.sessions.remove(&id);
            changes.push(Change::Remove(id));
        }
        changes
    }

    /// Earliest time at which [`Registry::tick`] can change something, so the
    /// caller can sleep until then instead of polling.
    pub fn next_deadline(&self) -> Option<Millis> {
        let c = &self.config;
        self.sessions
            .values()
            .filter_map(|s| {
                let since = s.view.since;
                match (s.pending, s.view.state) {
                    (_, BuddyState::Leaving) => Some(since + c.leave_ms),
                    (Some(_), state) => Some(since + c.min_for(state)),
                    (None, BuddyState::Spawning) => Some(since + c.spawn_ms),
                    (None, BuddyState::Idle) => Some(s.last_event_at + c.idle_timeout_ms),
                    _ => None,
                }
            })
            .min()
    }

    fn create(&mut self, event: RegistryEvent) -> Vec<Change> {
        if event.kind == EventKind::SessionEnded {
            return Vec::new();
        }
        let cwd = event.cwd.clone().unwrap_or_default();
        let mut session = Session {
            view: SessionView {
                id: event.session_id.clone(),
                project: project_name(&cwd),
                cwd,
                state: BuddyState::Spawning,
                tool: None,
                summary: None,
                since: event.at,
                source: event.source,
                pid: event.pid,
            },
            pending: None,
            last_event_at: event.at,
            created_at: event.at,
        };
        session.absorb_metadata(&event);
        // A session first seen mid-turn still spawns, then shows what it is doing.
        session.pending = session
            .target_state(&event.kind)
            .filter(|s| *s != BuddyState::Spawning);
        let view = session.view.clone();
        self.sessions.insert(event.session_id, session);
        vec![Change::Update(view)]
    }
}

impl Config {
    fn min_for(&self, state: BuddyState) -> Millis {
        match state {
            BuddyState::Spawning => self.spawn_ms,
            _ => self.min_state_ms,
        }
    }
}

impl Session {
    /// The state the event asks for, or `None` to keep the current one.
    fn target_state(&self, kind: &EventKind) -> Option<BuddyState> {
        let current = self.pending.unwrap_or(self.view.state);
        let target = match kind {
            EventKind::SessionStarted => None,
            EventKind::PromptSubmitted => Some(BuddyState::Working),
            EventKind::ToolStarted { tool, .. } => state_for_tool(tool).or(
                // A tool starting means the user answered, even for TodoWrite.
                (current == BuddyState::Waiting).then_some(BuddyState::Working),
            ),
            EventKind::ToolFinished { .. } => None,
            EventKind::NeedsInput => Some(BuddyState::Waiting),
            EventKind::TurnEnded => Some(BuddyState::Idle),
            EventKind::SessionEnded => Some(BuddyState::Leaving),
        };
        // Any activity wakes a sleeping buddy.
        target.or((current == BuddyState::Sleeping).then_some(BuddyState::Idle))
    }

    /// Copies cwd, pid, tool and summary from the event. Returns whether the view changed.
    fn absorb_metadata(&mut self, event: &RegistryEvent) -> bool {
        let before = self.view.clone();
        if let Some(cwd) = &event.cwd {
            if *cwd != self.view.cwd {
                self.view.project = project_name(cwd);
                self.view.cwd = cwd.clone();
            }
        }
        if event.pid.is_some() {
            self.view.pid = event.pid;
        }
        match &event.kind {
            EventKind::ToolStarted { tool, summary } => {
                self.view.tool = Some(tool.clone());
                self.view.summary = summary.as_deref().map(truncate);
            }
            EventKind::ToolFinished { tool } => self.view.tool = Some(tool.clone()),
            _ => {}
        }
        before != self.view
    }

    /// Moves to `target` now, or queues it until the current state has been
    /// visible for its minimum time. `Waiting` and `Leaving` never wait: the
    /// user must see them at once. Returns whether the view changed.
    fn request_state(&mut self, target: BuddyState, now: Millis, config: &Config) -> bool {
        if target == self.view.state {
            self.pending = None;
            return false;
        }
        let urgent = matches!(target, BuddyState::Waiting | BuddyState::Leaving);
        let shown_for = now.saturating_sub(self.view.since);
        if urgent || shown_for >= config.min_for(self.view.state) {
            self.view.state = target;
            self.view.since = now;
            self.pending = None;
            true
        } else {
            self.pending = Some(target);
            false
        }
    }
}

/// Last path segment of `cwd`, accepting both `/` and `\` separators.
fn project_name(cwd: &str) -> String {
    cwd.rsplit(['/', '\\'])
        .find(|s| !s.is_empty())
        .unwrap_or_default()
        .to_string()
}

/// Caps `text` at [`SUMMARY_MAX_CHARS`] characters, ending with `…` when cut.
fn truncate(text: &str) -> String {
    if text.chars().count() <= SUMMARY_MAX_CHARS {
        return text.to_string();
    }
    let mut cut: String = text.chars().take(SUMMARY_MAX_CHARS - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests;
