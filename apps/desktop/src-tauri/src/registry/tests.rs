use super::*;

const ID: &str = "11111111-1111-4111-8111-111111111111";

fn config() -> Config {
    Config {
        min_state_ms: 600,
        spawn_ms: 1_000,
        idle_timeout_ms: 10_000,
        leave_ms: 2_000,
        exit_grace_ms: 15_000,
        transcript_ttl_ms: 60_000,
    }
}

fn event(at: Millis, kind: EventKind) -> RegistryEvent {
    event_from(Source::Hooks, at, kind)
}

fn event_from(source: Source, at: Millis, kind: EventKind) -> RegistryEvent {
    RegistryEvent {
        session_id: ID.to_string(),
        source,
        at,
        cwd: Some("/home/user/projects/demo".to_string()),
        pid: Some(4242),
        kind,
    }
}

fn tool(name: &str) -> EventKind {
    EventKind::ToolStarted {
        tool: name.to_string(),
        summary: None,
        target: None,
    }
}

fn state(registry: &Registry) -> BuddyState {
    registry.snapshot()[0].state
}

/// Registry with one session that has finished spawning and is idle since t=1000.
fn idle_registry() -> Registry {
    let mut registry = Registry::new(config());
    registry.apply(event(0, EventKind::SessionStarted));
    registry.tick(1_000);
    assert_eq!(state(&registry), BuddyState::Idle);
    registry
}

#[test]
fn session_start_spawns_then_idles() {
    let mut registry = Registry::new(config());
    let changes = registry.apply(event(0, EventKind::SessionStarted));
    assert_eq!(changes.len(), 1);
    assert_eq!(state(&registry), BuddyState::Spawning);

    assert!(registry.tick(999).is_empty());
    registry.tick(1_000);
    assert_eq!(state(&registry), BuddyState::Idle);
}

#[test]
fn view_has_project_cwd_and_pid() {
    let registry = idle_registry();
    let view = &registry.snapshot()[0];
    assert_eq!(view.project, "demo");
    assert_eq!(view.cwd, "/home/user/projects/demo");
    assert_eq!(view.pid, Some(4242));
    assert_eq!(view.source, Source::Hooks);
}

#[test]
fn remembers_the_last_hook_event_only() {
    let mut registry = Registry::new(config());
    assert_eq!(registry.last_hook_event_at(), None);
    registry.apply(event_from(
        Source::Transcript,
        50,
        EventKind::SessionStarted,
    ));
    assert_eq!(registry.last_hook_event_at(), None);
    registry.apply(event(70, tool("Read")));
    assert_eq!(registry.last_hook_event_at(), Some(70));
}

#[test]
fn idle_timeout_can_change_while_running() {
    let mut registry = idle_registry(); // idle since t=1000, timeout 10 s
    registry.set_idle_timeout(2_000);
    registry.tick(3_000);
    assert_eq!(state(&registry), BuddyState::Sleeping);
}

#[test]
fn pid_lookup_for_window_focus() {
    let registry = idle_registry();
    assert_eq!(registry.pid(ID), Some(4242));
    assert_eq!(registry.pid("unknown"), None);
}

#[test]
fn project_name_handles_windows_paths() {
    assert_eq!(project_name(r"C:\Users\me\proj\"), "proj");
    assert_eq!(project_name("/home/user/demo"), "demo");
    assert_eq!(project_name(""), "");
}

#[test]
fn prompt_then_tools_switch_between_working_and_researching() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, EventKind::PromptSubmitted));
    assert_eq!(state(&registry), BuddyState::Working);

    registry.apply(event(3_000, tool("Read")));
    assert_eq!(state(&registry), BuddyState::Researching);

    registry.apply(event(4_000, tool("Edit")));
    assert_eq!(state(&registry), BuddyState::Working);
    assert_eq!(registry.snapshot()[0].tool.as_deref(), Some("Edit"));
}

#[test]
fn quick_flips_wait_for_minimum_display_time() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, tool("Read")));
    assert_eq!(state(&registry), BuddyState::Researching);

    // 100 ms later: too soon, queued.
    assert!(registry
        .apply(event(2_100, tool("Edit")))
        .iter()
        .all(|c| matches!(c, Change::Update(v) if v.state == BuddyState::Researching)));
    assert_eq!(state(&registry), BuddyState::Researching);
    assert_eq!(registry.next_deadline(), Some(2_600));

    registry.tick(2_599);
    assert_eq!(state(&registry), BuddyState::Researching);
    registry.tick(2_600);
    assert_eq!(state(&registry), BuddyState::Working);
}

#[test]
fn latest_queued_state_wins() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, tool("Read")));
    registry.apply(event(2_100, tool("Edit")));
    registry.apply(event(2_200, EventKind::TurnEnded));
    registry.tick(2_600);
    assert_eq!(state(&registry), BuddyState::Idle);
}

#[test]
fn todo_write_keeps_state() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, tool("Grep")));
    registry.apply(event(3_000, tool("TodoWrite")));
    assert_eq!(state(&registry), BuddyState::Researching);
}

#[test]
fn needs_input_shows_waiting_immediately() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, tool("Bash")));
    // Only 50 ms after the last change, but waiting is never delayed.
    registry.apply(event(2_050, EventKind::NeedsInput));
    assert_eq!(state(&registry), BuddyState::Waiting);
}

#[test]
fn waiting_ignores_tool_finished() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, EventKind::NeedsInput));
    registry.apply(event(
        3_000,
        EventKind::ToolFinished {
            tool: "Bash".into(),
        },
    ));
    assert_eq!(state(&registry), BuddyState::Waiting);
}

#[test]
fn waiting_left_on_tool_prompt_or_stop() {
    for kind in [tool("Bash"), tool("TodoWrite"), EventKind::PromptSubmitted] {
        let mut registry = idle_registry();
        registry.apply(event(2_000, EventKind::NeedsInput));
        registry.apply(event(3_000, kind.clone()));
        assert_eq!(state(&registry), BuddyState::Working, "{kind:?}");
    }

    let mut registry = idle_registry();
    registry.apply(event(2_000, EventKind::NeedsInput));
    registry.apply(event(3_000, EventKind::TurnEnded));
    assert_eq!(state(&registry), BuddyState::Idle);
}

#[test]
fn idle_falls_asleep_after_timeout_and_wakes_on_activity() {
    let mut registry = idle_registry();
    assert_eq!(registry.next_deadline(), Some(10_000));
    registry.tick(9_999);
    assert_eq!(state(&registry), BuddyState::Idle);
    registry.tick(10_000);
    assert_eq!(state(&registry), BuddyState::Sleeping);
    assert_eq!(registry.next_deadline(), None);

    registry.apply(event(
        20_000,
        EventKind::ToolFinished {
            tool: "Bash".into(),
        },
    ));
    assert_eq!(state(&registry), BuddyState::Idle);
}

#[test]
fn sleeping_wakes_straight_into_work() {
    let mut registry = idle_registry();
    registry.tick(10_000);
    registry.apply(event(20_000, EventKind::PromptSubmitted));
    assert_eq!(state(&registry), BuddyState::Working);
}

#[test]
fn session_end_leaves_then_removes() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, EventKind::SessionEnded));
    assert_eq!(state(&registry), BuddyState::Leaving);

    // Leaving is final.
    registry.apply(event(2_100, EventKind::PromptSubmitted));
    assert_eq!(state(&registry), BuddyState::Leaving);

    assert!(registry.tick(3_999).is_empty());
    assert_eq!(registry.tick(4_000), vec![Change::Remove(ID.to_string())]);
    assert!(registry.snapshot().is_empty());
}

#[test]
fn session_end_for_unknown_session_is_ignored() {
    let mut registry = Registry::new(config());
    assert!(registry.apply(event(0, EventKind::SessionEnded)).is_empty());
    assert!(registry.snapshot().is_empty());
}

#[test]
fn session_first_seen_mid_turn_spawns_then_shows_its_state() {
    let mut registry = Registry::new(config());
    registry.apply(event(0, tool("WebSearch")));
    assert_eq!(state(&registry), BuddyState::Spawning);
    registry.tick(1_000);
    assert_eq!(state(&registry), BuddyState::Researching);
}

#[test]
fn hooks_win_over_transcript() {
    let mut registry = Registry::new(config());
    registry.apply(event_from(Source::Transcript, 0, EventKind::SessionStarted));
    registry.tick(1_000);
    assert_eq!(registry.snapshot()[0].source, Source::Transcript);

    registry.apply(event(2_000, tool("Read")));
    assert_eq!(registry.snapshot()[0].source, Source::Hooks);

    // Transcript inference is now ignored.
    let changes = registry.apply(event_from(Source::Transcript, 3_000, EventKind::TurnEnded));
    assert!(changes.is_empty());
    assert_eq!(state(&registry), BuddyState::Researching);
}

#[test]
fn summary_is_capped_at_80_chars() {
    let mut registry = idle_registry();
    registry.apply(event(
        2_000,
        EventKind::ToolStarted {
            tool: "Bash".into(),
            summary: Some("x".repeat(200)),
            target: None,
        },
    ));
    let summary = registry.snapshot()[0].summary.clone().unwrap();
    assert_eq!(summary.chars().count(), 80);
    assert!(summary.ends_with('…'));
}

#[test]
fn snapshot_keeps_creation_order() {
    let mut registry = Registry::new(config());
    for (i, id) in ["b", "a", "c"].into_iter().enumerate() {
        registry.apply(RegistryEvent {
            session_id: id.into(),
            ..event(i as Millis, EventKind::SessionStarted)
        });
    }
    let ids: Vec<_> = registry.snapshot().into_iter().map(|v| v.id).collect();
    assert_eq!(ids, ["b", "a", "c"]);
}

#[test]
fn view_serialises_like_the_typescript_contract() {
    let registry = idle_registry();
    let json = serde_json::to_value(&registry.snapshot()[0]).unwrap();
    assert_eq!(
        json,
        serde_json::json!({
            "id": ID,
            "project": "demo",
            "cwd": "/home/user/projects/demo",
            "state": "idle",
            "since": 1000,
            "source": "hooks",
            "pid": 4242
        })
    );
}

/// Minimal hook → event mapping, only to prove the captured payloads drive the
/// state machine as documented. The real adapter lives in the bridge (#12).
fn from_hook_fixture(name: &str, at: Millis) -> RegistryEvent {
    let path = format!(
        "{}/../../../tests/fixtures/hooks/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let payload: serde_json::Value = serde_json::from_str(&text).unwrap();
    let tool = || payload["tool_name"].as_str().unwrap().to_string();
    let kind = match payload["hook_event_name"].as_str().unwrap() {
        "SessionStart" => EventKind::SessionStarted,
        "UserPromptSubmit" => EventKind::PromptSubmitted,
        "PreToolUse" => EventKind::ToolStarted {
            tool: tool(),
            summary: None,
            target: None,
        },
        "PostToolUse" => EventKind::ToolFinished { tool: tool() },
        "Notification" => EventKind::NeedsInput,
        "Stop" => EventKind::TurnEnded,
        "SessionEnd" => EventKind::SessionEnded,
        other => panic!("unmapped hook event {other}"),
    };
    RegistryEvent {
        session_id: payload["session_id"].as_str().unwrap().to_string(),
        source: Source::Hooks,
        at,
        cwd: payload["cwd"].as_str().map(str::to_string),
        pid: None,
        kind,
    }
}

#[test]
fn captured_session_plays_out_as_documented() {
    let mut registry = Registry::new(config());
    let steps = [
        ("captured/session-start.json", BuddyState::Spawning),
        ("captured/user-prompt-submit.json", BuddyState::Working),
        ("captured/pre-tool-use-read.json", BuddyState::Researching),
        ("captured/post-tool-use-read.json", BuddyState::Researching),
        ("captured/pre-tool-use-bash.json", BuddyState::Working),
        (
            "from-docs/notification-permission-prompt.json",
            BuddyState::Waiting,
        ),
        ("captured/post-tool-use-bash.json", BuddyState::Waiting),
        ("captured/pre-tool-use-agent.json", BuddyState::Working),
        ("captured/stop.json", BuddyState::Idle),
        ("captured/session-end.json", BuddyState::Leaving),
    ];
    // Steps 1 s apart; each is checked 700 ms in, once the minimum display
    // time of the previous state has passed.
    for (i, (fixture, expected)) in steps.into_iter().enumerate() {
        let at = i as Millis * 1_000;
        registry.apply(from_hook_fixture(fixture, at));
        registry.tick(at + 700);
        assert_eq!(state(&registry), expected, "after {fixture}");
    }
    assert_eq!(registry.snapshot()[0].project, "demo");
}

#[test]
fn exited_process_leaves_after_grace() {
    let mut registry = idle_registry();
    registry.process_exited(ID, 4242, 2_000);
    assert_eq!(
        registry.next_deadline(),
        Some(10_000),
        "idle timeout comes first"
    );

    registry.tick(16_999);
    assert_ne!(state(&registry), BuddyState::Leaving);
    registry.tick(17_000);
    assert_eq!(state(&registry), BuddyState::Leaving);
}

#[test]
fn exit_deadline_is_reported() {
    let mut registry = idle_registry();
    registry.apply(event(2_000, tool("Bash")));
    registry.process_exited(ID, 4242, 3_000);
    assert_eq!(registry.next_deadline(), Some(18_000));
}

#[test]
fn exit_of_another_pid_is_ignored() {
    let mut registry = idle_registry();
    registry.process_exited(ID, 1, 2_000);
    registry.process_exited("unknown-session", 4242, 2_000);
    registry.tick(20_000);
    assert_ne!(state(&registry), BuddyState::Leaving);
}

#[test]
fn new_process_cancels_the_exit() {
    let mut registry = idle_registry();
    registry.process_exited(ID, 4242, 2_000);
    // Resumed in a new process within the grace period.
    registry.apply(RegistryEvent {
        pid: Some(5000),
        ..event(5_000, EventKind::SessionStarted)
    });
    registry.tick(30_000);
    assert_ne!(state(&registry), BuddyState::Leaving);
    assert_eq!(registry.snapshot()[0].pid, Some(5000));
}

#[test]
fn transcript_only_session_expires_without_writes() {
    let mut registry = Registry::new(config());
    registry.apply(event_from(Source::Transcript, 0, EventKind::SessionStarted));
    registry.apply(event_from(Source::Transcript, 30_000, tool("Read")));
    registry.tick(89_999);
    assert_ne!(state(&registry), BuddyState::Leaving);
    registry.tick(90_000);
    assert_eq!(state(&registry), BuddyState::Leaving);
}

#[test]
fn hook_sessions_do_not_expire_on_silence() {
    let mut registry = idle_registry();
    registry.tick(10_000_000);
    assert_eq!(state(&registry), BuddyState::Sleeping);
}

// --- Sessions talking to each other ---------------------------------------

const ALICE: &str = "aaaaaaaa-1111-4111-8111-111111111111";
const BOB: &str = "bbbbbbbb-2222-4222-8222-222222222222";
const CAROL: &str = "cccccccc-3333-4333-8333-333333333333";

fn event_for(id: &str, project: &str, at: Millis, kind: EventKind) -> RegistryEvent {
    RegistryEvent {
        session_id: id.to_string(),
        source: Source::Hooks,
        at,
        cwd: Some(format!("/home/user/{project}")),
        pid: None,
        kind,
    }
}

fn send_message(to: Option<&str>) -> EventKind {
    EventKind::ToolStarted {
        tool: "SendMessage".into(),
        summary: Some("SendMessage".into()),
        target: to.map(str::to_string),
    }
}

/// Registry with Alice (api), Bob (web) and, optionally, Carol (docs).
fn chatty(with_carol: bool) -> Registry {
    let mut registry = Registry::new(config());
    registry.apply(event_for(ALICE, "api", 0, EventKind::SessionStarted));
    registry.apply(event_for(BOB, "web", 0, EventKind::SessionStarted));
    if with_carol {
        registry.apply(event_for(CAROL, "docs", 0, EventKind::SessionStarted));
    }
    registry.tick(1_000);
    registry
}

fn interactions(changes: &[Change]) -> Vec<Interaction> {
    changes
        .iter()
        .filter_map(|c| match c {
            Change::Interaction(i) => Some(i.clone()),
            _ => None,
        })
        .collect()
}

fn talk(from: &str, to: Option<&str>) -> Interaction {
    Interaction {
        from: from.to_string(),
        to: to.map(str::to_string),
    }
}

#[test]
fn a_message_to_a_known_session_pairs_at_once() {
    let mut registry = chatty(true);
    // By id, by the start of the id, and by project name.
    for (name, to) in [(BOB, BOB), ("cccccccc-3333", CAROL), ("WEB", BOB)] {
        let changes = registry.apply(event_for(ALICE, "api", 2_000, send_message(Some(name))));
        assert_eq!(interactions(&changes), [talk(ALICE, Some(to))], "{name}");
    }
    assert!(registry.messages.is_empty(), "nothing left waiting");
}

#[test]
fn the_session_that_gets_a_prompt_next_is_the_recipient() {
    let mut registry = chatty(true);
    let sent = registry.apply(event_for(ALICE, "api", 2_000, send_message(Some("helper"))));
    assert!(interactions(&sent).is_empty(), "waits for the recipient");
    assert_eq!(registry.next_deadline(), Some(2_000 + MESSAGE_WINDOW_MS));
    // A prompt in the sender itself proves nothing.
    let own = registry.apply(event_for(ALICE, "api", 2_500, EventKind::PromptSubmitted));
    assert!(interactions(&own).is_empty());
    let got = registry.apply(event_for(CAROL, "docs", 3_000, EventKind::PromptSubmitted));
    assert_eq!(interactions(&got), [talk(ALICE, Some(CAROL))]);
    assert!(
        interactions(&registry.tick(10_000)).is_empty(),
        "settled once"
    );
}

#[test]
fn with_one_other_session_it_is_the_recipient() {
    let mut registry = chatty(false);
    registry.apply(event_for(ALICE, "api", 2_000, send_message(Some("helper"))));
    assert_eq!(registry.next_deadline(), Some(2_000 + LONE_PAIR_MS));
    assert!(interactions(&registry.tick(2_000 + LONE_PAIR_MS - 1)).is_empty());
    assert_eq!(
        interactions(&registry.tick(2_000 + LONE_PAIR_MS)),
        [talk(ALICE, Some(BOB))]
    );
}

#[test]
fn an_unpaired_message_is_reported_without_a_recipient() {
    let mut registry = chatty(true);
    registry.apply(event_for(ALICE, "api", 2_000, send_message(None)));
    // Too late for a prompt to count.
    let late = registry.apply(event_for(
        BOB,
        "web",
        2_000 + MESSAGE_WINDOW_MS + 1,
        EventKind::PromptSubmitted,
    ));
    assert!(interactions(&late).is_empty());
    assert_eq!(
        interactions(&registry.tick(2_000 + MESSAGE_WINDOW_MS + 1)),
        [talk(ALICE, None)]
    );
}

#[test]
fn ordinary_tools_are_not_messages() {
    let mut registry = chatty(false);
    let changes = registry.apply(event_for(ALICE, "api", 2_000, tool("Read")));
    assert!(interactions(&changes).is_empty());
    assert!(registry.messages.is_empty());
}
