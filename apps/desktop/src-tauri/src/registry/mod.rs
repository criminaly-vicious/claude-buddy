//! Session registry: the single source of truth for every Claude Code session
//! and the state its buddy shows. Pure logic with no I/O: callers pass the
//! current time in, so every transition is deterministic and testable.
//!
//! Inputs are normalised [`RegistryEvent`]s (adapted from hooks by the bridge
//! and from transcripts by the watcher). Outputs are [`Change`]s, which the app
//! forwards to the overlay as `sessions:update` / `sessions:remove` /
//! `sessions:interaction` events. See docs/session-detection.md#state-machine,
//! #subagents and #sessions-talking-to-each-other.

mod tool_map;

use std::collections::HashMap;

use bridge_protocol::target::is_message_tool;
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
/// Tauri event carrying an [`Interaction`]: one session messaged another.
pub const EVENT_INTERACTION: &str = "sessions:interaction";

/// How long a sent message waits for its recipient to show up.
const MESSAGE_WINDOW_MS: Millis = 5_000;
/// With only one other session around, it is the recipient after this long.
const LONE_PAIR_MS: Millis = 1_500;

/// A subagent silent for this long is taken to be gone, in case its
/// `SubagentStop` never came (an interrupted turn, the app restarting).
pub const SUBAGENT_TTL_MS: Millis = 10 * 60 * 1_000;
/// The same for a subagent known only from its tools: hooks installed before
/// subagents were followed send no `SubagentStart` or `SubagentStop`.
pub const UNANNOUNCED_SUBAGENT_TTL_MS: Millis = 90 * 1_000;
/// Subagents tracked per session, at most.
const MAX_SUBAGENTS: usize = 16;

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
    /// `summary` and `target` must already be sanitised by the source
    /// (tool name + short target; for messaging tools the recipient).
    ToolStarted {
        tool: String,
        summary: Option<String>,
        target: Option<String>,
    },
    ToolFinished {
        tool: String,
    },
    /// Permission prompt, question or idle prompt: the user is needed.
    NeedsInput,
    TurnEnded,
    /// A subagent started; [`RegistryEvent::agent`] names it.
    SubagentStarted,
    /// A subagent finished; [`RegistryEvent::agent`] names it.
    SubagentStopped,
    SessionEnded,
}

/// The subagent an event comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Agent {
    /// Claude Code's `agent_id`.
    pub id: String,
    /// Its `agent_type`, e.g. `Explore`.
    pub kind: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryEvent {
    pub session_id: String,
    pub source: Source,
    pub at: Millis,
    pub cwd: Option<String>,
    pub pid: Option<u32>,
    /// Set when the event comes from inside a subagent.
    pub agent: Option<Agent>,
    pub kind: EventKind,
}

/// A running subagent, shown as a mini buddy on its session's buddy.
/// Mirrors `SubagentView` in apps/desktop/src/types.ts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubagentView {
    pub id: String,
    /// Its type, e.g. `Explore`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub since: Millis,
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
    /// Subagents running in this session, oldest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub subagents: Vec<SubagentView>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Change {
    Update(SessionView),
    Remove(String),
    /// One session messaged another (`to`), or someone the registry could
    /// not tell (`to: None`).
    Interaction(Interaction),
}

/// Payload of [`EVENT_INTERACTION`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Interaction {
    pub from: String,
    pub to: Option<String>,
}

/// A message a session sent, waiting to be paired with its recipient.
#[derive(Clone, Debug)]
struct Sent {
    from: String,
    at: Millis,
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
    /// How long a session stays after its `claude` process exits, in case it
    /// comes back (e.g. resumed) before the buddy leaves.
    pub exit_grace_ms: Millis,
    /// Transcript-only sessions leave after this long without writes.
    pub transcript_ttl_ms: Millis,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_state_ms: 600,
            spawn_ms: 1_000,
            idle_timeout_ms: 5 * 60 * 1_000,
            leave_ms: 2_000,
            exit_grace_ms: 15_000,
            transcript_ttl_ms: 30 * 60 * 1_000,
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
    /// When the session's `claude` process was seen exiting.
    exited_at: Option<Millis>,
    /// When each running subagent was last heard from, by id.
    subagents_seen: HashMap<String, Seen>,
}

/// When a subagent was last heard from, and whether its start was reported.
#[derive(Clone, Copy, Debug)]
struct Seen {
    at: Millis,
    announced: bool,
}

impl Seen {
    /// When the subagent counts as gone if nothing more is heard.
    fn gone_at(&self) -> Millis {
        let ttl = if self.announced {
            SUBAGENT_TTL_MS
        } else {
            UNANNOUNCED_SUBAGENT_TTL_MS
        };
        self.at + ttl
    }
}

#[derive(Debug, Default)]
pub struct Registry {
    config: Config,
    sessions: HashMap<String, Session>,
    /// When the last hook event arrived, for the settings window's status.
    last_hook_event_at: Option<Millis>,
    /// Messages whose recipient is not known yet, oldest first.
    messages: Vec<Sent>,
}

impl Registry {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            sessions: HashMap::new(),
            last_hook_event_at: None,
            messages: Vec::new(),
        }
    }

    /// Changes the idle → sleeping timeout (settings). Takes effect at the
    /// next tick; the driver must be woken to recompute its deadline.
    pub fn set_idle_timeout(&mut self, ms: Millis) {
        self.config.idle_timeout_ms = ms;
    }

    /// When the last event from a hook arrived, if any since start.
    pub fn last_hook_event_at(&self) -> Option<Millis> {
        self.last_hook_event_at
    }

    /// All live sessions, oldest first (stable slot order for the overlay).
    pub fn snapshot(&self) -> Vec<SessionView> {
        let mut sessions: Vec<&Session> = self.sessions.values().collect();
        sessions.sort_by(|a, b| (a.created_at, &a.view.id).cmp(&(b.created_at, &b.view.id)));
        sessions.into_iter().map(|s| s.view.clone()).collect()
    }

    /// The process behind session `id`, for window focus. Only sessions the
    /// registry knows can be focused, whatever the overlay asks for.
    pub fn pid(&self, id: &str) -> Option<u32> {
        self.sessions.get(id).and_then(|s| s.view.pid)
    }

    /// Applies one event and returns what changed.
    pub fn apply(&mut self, event: RegistryEvent) -> Vec<Change> {
        if event.source == Source::Hooks {
            self.last_hook_event_at = Some(event.at);
        }
        let config = self.config;
        let Some(session) = self.sessions.get_mut(&event.session_id) else {
            let talk = self.track_messages(&event);
            let mut changes = self.create(event);
            changes.extend(talk);
            return changes;
        };

        // Hooks are exact; once a session has them, transcript inference is noise.
        if event.source == Source::Transcript && session.view.source == Source::Hooks {
            return Vec::new();
        }
        if session.view.state == BuddyState::Leaving {
            return Vec::new();
        }

        // A new process for the same session (e.g. `claude --resume`) means it is alive again.
        if event.pid.is_some() && event.pid != session.view.pid {
            session.exited_at = None;
        }
        let mut changed = false;
        if event.source == Source::Hooks && session.view.source == Source::Transcript {
            session.view.source = Source::Hooks;
            changed = true;
        }
        changed |= session.absorb_metadata(&event);
        changed |= session.track_subagent(&event);
        session.last_event_at = event.at;

        let target = session.target_state(&event);
        if let Some(target) = target {
            changed |= session.request_state(target, event.at, &config);
        }
        let mut changes = if changed {
            vec![Change::Update(session.view.clone())]
        } else {
            Vec::new()
        };
        changes.extend(self.track_messages(&event));
        changes
    }

    /// Pairs messages between sessions (docs/session-detection.md#sessions-talking-to-each-other).
    ///
    /// A messaging tool names its recipient (`target`): a session id or a
    /// project name pairs at once. Otherwise the message waits: the next
    /// other session to receive a prompt within [`MESSAGE_WINDOW_MS`] is the
    /// recipient, or the only other session after [`LONE_PAIR_MS`]; failing
    /// both, the interaction is reported without a recipient.
    fn track_messages(&mut self, event: &RegistryEvent) -> Vec<Change> {
        match &event.kind {
            EventKind::ToolStarted { tool, target, .. } if is_message_tool(tool) => {
                let named = target
                    .as_deref()
                    .and_then(|name| self.session_named(name, &event.session_id));
                match named {
                    Some(to) => vec![Change::Interaction(Interaction {
                        from: event.session_id.clone(),
                        to: Some(to),
                    })],
                    None => {
                        self.messages.push(Sent {
                            from: event.session_id.clone(),
                            at: event.at,
                        });
                        Vec::new()
                    }
                }
            }
            EventKind::PromptSubmitted => {
                let received = self.messages.iter().position(|m| {
                    m.from != event.session_id && event.at.saturating_sub(m.at) <= MESSAGE_WINDOW_MS
                });
                received
                    .map(|i| {
                        let sent = self.messages.remove(i);
                        Change::Interaction(Interaction {
                            from: sent.from,
                            to: Some(event.session_id.clone()),
                        })
                    })
                    .into_iter()
                    .collect()
            }
            _ => Vec::new(),
        }
    }

    /// The session (other than `from`) that `name` refers to: its id, the
    /// start of its id (8 characters or more), or its project name.
    fn session_named(&self, name: &str, from: &str) -> Option<String> {
        let name = name.trim().to_lowercase();
        if name.is_empty() {
            return None;
        }
        let mut candidates: Vec<&Session> = self
            .sessions
            .values()
            .filter(|s| s.view.id != from && s.view.state != BuddyState::Leaving)
            .collect();
        candidates.sort_by(|a, b| (a.created_at, &a.view.id).cmp(&(b.created_at, &b.view.id)));
        let id = |s: &&Session| s.view.id.to_lowercase();
        candidates
            .iter()
            .find(|s| id(s) == name)
            .or_else(|| {
                candidates
                    .iter()
                    .find(|s| name.len() >= 8 && id(s).starts_with(&name))
            })
            .or_else(|| {
                candidates
                    .iter()
                    .find(|s| s.view.project.to_lowercase() == name)
            })
            .map(|s| s.view.id.clone())
    }

    /// The other live sessions a message from `from` could be for.
    fn others(&self, from: &str) -> Vec<&Session> {
        self.sessions
            .values()
            .filter(|s| s.view.id != from && s.view.state != BuddyState::Leaving)
            .collect()
    }

    /// Settles messages that waited long enough (see [`Registry::track_messages`]).
    fn settle_messages(&mut self, now: Millis) -> Vec<Change> {
        let mut changes = Vec::new();
        let mut waiting = Vec::new();
        for sent in std::mem::take(&mut self.messages) {
            if !self.sessions.contains_key(&sent.from) {
                continue;
            }
            let age = now.saturating_sub(sent.at);
            let others = self.others(&sent.from);
            if age >= LONE_PAIR_MS && others.len() == 1 {
                let to = others[0].view.id.clone();
                changes.push(Change::Interaction(Interaction {
                    from: sent.from,
                    to: Some(to),
                }));
            } else if age >= MESSAGE_WINDOW_MS {
                changes.push(Change::Interaction(Interaction {
                    from: sent.from,
                    to: None,
                }));
            } else {
                waiting.push(sent);
            }
        }
        self.messages = waiting;
        changes
    }

    /// When the oldest waiting message settles, if any.
    fn messages_deadline(&self) -> Option<Millis> {
        self.messages
            .iter()
            .map(|m| {
                if self.others(&m.from).len() == 1 {
                    m.at + LONE_PAIR_MS
                } else {
                    m.at + MESSAGE_WINDOW_MS
                }
            })
            .min()
    }

    /// Records that process `pid` of a session exited. The buddy leaves after
    /// [`Config::exit_grace_ms`] unless the session shows a new process first.
    /// Ignored when `pid` is not the session's current process.
    pub fn process_exited(&mut self, session_id: &str, pid: u32, at: Millis) {
        if let Some(session) = self.sessions.get_mut(session_id) {
            if session.view.pid == Some(pid) && session.exited_at.is_none() {
                session.exited_at = Some(at);
            }
        }
    }

    /// Advances timers (pending states, spawn end, sleep, expiry, removal).
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

            let dropped = session.drop_silent_subagents(now);
            let next = if session.is_expired(now, &config) {
                Some(BuddyState::Leaving)
            } else if let Some(pending) = session.pending {
                (elapsed >= config.min_for(session.view.state)).then_some(pending)
            } else if session.view.state == BuddyState::Spawning {
                (elapsed >= config.spawn_ms).then_some(BuddyState::Idle)
            } else if session.view.state == BuddyState::Idle {
                let quiet = now.saturating_sub(session.last_event_at);
                (quiet >= config.idle_timeout_ms).then_some(BuddyState::Sleeping)
            } else {
                None
            };

            let moved = next.is_some_and(|next| session.request_state(next, now, &config));
            if moved || dropped {
                changes.push(Change::Update(session.view.clone()));
            }
        }

        removed.sort();
        for id in removed {
            self.sessions.remove(&id);
            changes.push(Change::Remove(id));
        }
        changes.extend(self.settle_messages(now));
        changes
    }

    /// Earliest time at which [`Registry::tick`] can change something, so the
    /// caller can sleep until then instead of polling.
    pub fn next_deadline(&self) -> Option<Millis> {
        let c = &self.config;
        let sessions = self
            .sessions
            .values()
            .filter_map(|s| {
                let since = s.view.since;
                let state_deadline = match (s.pending, s.view.state) {
                    (_, BuddyState::Leaving) => return Some(since + c.leave_ms),
                    (Some(_), state) => Some(since + c.min_for(state)),
                    (None, BuddyState::Spawning) => Some(since + c.spawn_ms),
                    (None, BuddyState::Idle) => Some(s.last_event_at + c.idle_timeout_ms),
                    _ => None,
                };
                let silent = s.subagents_seen.values().map(Seen::gone_at).min();
                [state_deadline, s.expiry_deadline(c), silent]
                    .into_iter()
                    .flatten()
                    .min()
            })
            .min();
        [sessions, self.messages_deadline()]
            .into_iter()
            .flatten()
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
                subagents: Vec::new(),
            },
            pending: None,
            last_event_at: event.at,
            created_at: event.at,
            exited_at: None,
            subagents_seen: HashMap::new(),
        };
        session.absorb_metadata(&event);
        session.track_subagent(&event);
        // A session first seen mid-turn still spawns, then shows what it is doing.
        session.pending = session
            .target_state(&event)
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
    /// When this session expires: after its process exit grace, or after the
    /// transcript TTL for sessions known only from transcripts.
    fn expiry_deadline(&self, config: &Config) -> Option<Millis> {
        let exit = self.exited_at.map(|at| at + config.exit_grace_ms);
        let stale = (self.view.source == Source::Transcript)
            .then(|| self.last_event_at + config.transcript_ttl_ms);
        [exit, stale].into_iter().flatten().min()
    }

    fn is_expired(&self, now: Millis, config: &Config) -> bool {
        self.expiry_deadline(config)
            .is_some_and(|deadline| now >= deadline)
    }

    /// The state the event asks for, or `None` to keep the current one.
    fn target_state(&self, event: &RegistryEvent) -> Option<BuddyState> {
        let current = self.pending.unwrap_or(self.view.state);
        // A tool starting means the user answered, even for TodoWrite.
        let answered = (current == BuddyState::Waiting).then_some(BuddyState::Working);
        let from_subagent = event.agent.is_some();
        let target = match &event.kind {
            EventKind::SessionStarted => None,
            EventKind::PromptSubmitted => Some(BuddyState::Working),
            // A subagent's tools show on its mini buddy, not on the parent.
            EventKind::ToolStarted { .. } if from_subagent => answered,
            EventKind::ToolStarted { tool, .. } => state_for_tool(tool).or(answered),
            EventKind::ToolFinished { .. } => None,
            EventKind::NeedsInput => Some(BuddyState::Waiting),
            // A subagent finishing is not the end of the parent's turn.
            EventKind::TurnEnded if from_subagent => None,
            EventKind::TurnEnded => Some(BuddyState::Idle),
            EventKind::SubagentStarted | EventKind::SubagentStopped => None,
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
        // A subagent's tools are its own (see `track_subagent`).
        if event.agent.is_none() {
            match &event.kind {
                EventKind::ToolStarted { tool, summary, .. } => {
                    self.view.tool = Some(tool.clone());
                    self.view.summary = summary.as_deref().map(truncate);
                }
                EventKind::ToolFinished { tool } => self.view.tool = Some(tool.clone()),
                _ => {}
            }
        }
        before != self.view
    }

    /// Starts, updates or ends the subagent an event comes from
    /// (docs/session-detection.md#subagents). A subagent starts with
    /// `SubagentStart` or its first tool, so hooks that miss the start still
    /// show it. Returns whether the view changed.
    fn track_subagent(&mut self, event: &RegistryEvent) -> bool {
        let Some(agent) = &event.agent else {
            return false;
        };
        if matches!(
            event.kind,
            EventKind::SubagentStopped | EventKind::TurnEnded
        ) {
            self.subagents_seen.remove(&agent.id);
            let before = self.view.subagents.len();
            self.view.subagents.retain(|s| s.id != agent.id);
            return self.view.subagents.len() != before;
        }
        if event.kind == EventKind::SessionEnded || self.view.state == BuddyState::Leaving {
            return false;
        }
        let (index, added) = match self.view.subagents.iter().position(|s| s.id == agent.id) {
            Some(index) => (index, false),
            None if self.view.subagents.len() < MAX_SUBAGENTS => {
                self.view.subagents.push(SubagentView {
                    id: agent.id.clone(),
                    kind: None,
                    tool: None,
                    summary: None,
                    since: event.at,
                });
                (self.view.subagents.len() - 1, true)
            }
            None => return false,
        };
        let announced = event.kind == EventKind::SubagentStarted
            || self
                .subagents_seen
                .get(&agent.id)
                .is_some_and(|s| s.announced);
        self.subagents_seen.insert(
            agent.id.clone(),
            Seen {
                at: event.at,
                announced,
            },
        );
        let subagent = &mut self.view.subagents[index];
        let before = subagent.clone();
        if agent.kind.is_some() {
            subagent.kind = agent.kind.clone();
        }
        match &event.kind {
            EventKind::ToolStarted { tool, summary, .. } => {
                subagent.tool = Some(tool.clone());
                subagent.summary = summary.as_deref().map(truncate);
            }
            EventKind::ToolFinished { tool } => subagent.tool = Some(tool.clone()),
            _ => {}
        }
        added || *subagent != before
    }

    /// Drops subagents silent for too long (see [`Seen::gone_at`]). Returns
    /// whether any went.
    fn drop_silent_subagents(&mut self, now: Millis) -> bool {
        self.subagents_seen.retain(|_, seen| now < seen.gone_at());
        let seen = &self.subagents_seen;
        let before = self.view.subagents.len();
        self.view.subagents.retain(|s| seen.contains_key(&s.id));
        self.view.subagents.len() != before
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
            if target == BuddyState::Leaving {
                // The minis go with their buddy.
                self.view.subagents.clear();
                self.subagents_seen.clear();
            }
            true
        } else {
            self.pending = Some(target);
            false
        }
    }
}

/// Hover-card line for a tool call: `"Read · index.ts"`, or just the tool.
pub fn tool_summary(tool: &str, target: Option<&str>) -> String {
    match target {
        Some(target) => format!("{tool} · {target}"),
        None => tool.to_string(),
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
