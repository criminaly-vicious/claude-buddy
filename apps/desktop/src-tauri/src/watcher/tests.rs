//! Integration tests: a real watcher on a temporary `projects` folder.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::time::Duration;

use super::*;

const SESSION: &str = "11111111-1111-4111-8111-111111111111";

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../../tests/fixtures/transcripts/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn temp_projects(name: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("claude-buddy-watch-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let projects = dir.join("projects");
    fs::create_dir_all(projects.join("-home-user-projects-demo")).unwrap();
    projects
}

fn append(path: &Path, text: &str) {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
    file.sync_all().unwrap();
}

fn clock() -> Millis {
    7
}

/// Collects event kinds until `count` arrive or the channel goes quiet.
fn events(rx: &Receiver<Input>, count: usize) -> Vec<RegistryEvent> {
    let mut out = Vec::new();
    while out.len() < count {
        match rx.recv_timeout(Duration::from_secs(5)) {
            Ok(Input::Event(event)) => out.push(event),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    out
}

#[test]
fn appended_lines_become_transcript_events() {
    let projects = temp_projects("append");
    let (tx, rx) = mpsc::channel();
    let _watcher = TranscriptWatcher::start(&projects, tx, clock).unwrap();

    let file = projects
        .join("-home-user-projects-demo")
        .join(format!("{SESSION}.jsonl"));
    append(&file, &fixture("session.jsonl"));

    // 9 signals: the final text and `stop_hook_summary` both end the turn.
    let got = events(&rx, 9);
    let kinds: Vec<_> = got.iter().map(|e| e.kind.clone()).collect();
    assert_eq!(kinds.len(), 9, "{kinds:?}");
    assert_eq!(kinds[0], EventKind::PromptSubmitted);
    assert_eq!(
        kinds[1],
        EventKind::ToolStarted {
            tool: "Read".into(),
            summary: Some("Read · notes.txt".into())
        }
    );
    assert_eq!(
        kinds[3],
        EventKind::ToolFinished {
            tool: "Bash".into()
        }
    );
    assert_eq!(kinds.last(), Some(&EventKind::TurnEnded));

    let first = &got[0];
    assert_eq!(first.session_id, SESSION);
    assert_eq!(first.source, Source::Transcript);
    assert_eq!(first.cwd.as_deref(), Some("/home/user/projects/demo"));
    assert_eq!(first.at, 7);
    assert_eq!(first.pid, None);
    let _ = fs::remove_dir_all(projects.parent().unwrap());
}

#[test]
fn subagent_transcripts_produce_nothing() {
    let projects = temp_projects("subagent");
    let (tx, rx) = mpsc::channel();
    let _watcher = TranscriptWatcher::start(&projects, tx, clock).unwrap();

    let dir = projects
        .join("-home-user-projects-demo")
        .join(SESSION)
        .join("subagents");
    fs::create_dir_all(&dir).unwrap();
    append(
        &dir.join("agent-a0000000000000001.jsonl"),
        &fixture("subagents/agent-a0000000000000001.jsonl"),
    );
    assert!(rx.recv_timeout(Duration::from_millis(800)).is_err());
    let _ = fs::remove_dir_all(projects.parent().unwrap());
}

#[test]
fn recent_files_are_read_at_startup_and_old_ones_skipped() {
    let projects = temp_projects("startup");
    let folder = projects.join("-home-user-projects-demo");
    let recent = folder.join(format!("{SESSION}.jsonl"));
    append(&recent, &fixture("session.jsonl"));
    let old = folder.join("22222222-2222-4222-8222-222222222222.jsonl");
    append(
        &old,
        &fixture("session.jsonl").replace(SESSION, "old-session"),
    );
    let hour_ago = SystemTime::now() - Duration::from_secs(60 * 60);
    OpenOptions::new()
        .append(true)
        .open(&old)
        .unwrap()
        .set_modified(hour_ago)
        .unwrap();

    let (tx, rx) = mpsc::channel();
    let _watcher = TranscriptWatcher::start(&projects, tx, clock).unwrap();
    let got = events(&rx, 9);
    assert_eq!(got.len(), 9);
    assert!(got.iter().all(|e| e.session_id == SESSION));
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "old file not read"
    );
    let _ = fs::remove_dir_all(projects.parent().unwrap());
}

#[test]
fn only_new_lines_are_read() {
    let projects = temp_projects("incremental");
    let file = projects
        .join("-home-user-projects-demo")
        .join(format!("{SESSION}.jsonl"));
    let text = fixture("session.jsonl");
    let lines: Vec<&str> = text.lines().collect();
    // First turn up to the Read tool call, already on disk at startup.
    append(&file, &format!("{}\n", lines[..7].join("\n")));

    let (tx, rx) = mpsc::channel();
    let _watcher = TranscriptWatcher::start(&projects, tx, clock).unwrap();
    assert_eq!(events(&rx, 2).len(), 2, "prompt + Read");

    append(&file, &format!("{}\n", lines[15]));
    let got = events(&rx, 1);
    assert_eq!(got[0].kind, EventKind::TurnEnded);
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "no replay"
    );
    let _ = fs::remove_dir_all(projects.parent().unwrap());
}

#[test]
fn transcript_detection() {
    assert!(is_transcript(Path::new("/p/-demo/abc.jsonl")));
    assert!(!is_transcript(Path::new(
        "/p/-demo/abc/subagents/agent-1.jsonl"
    )));
    assert!(!is_transcript(Path::new(
        "/p/-demo/abc/subagents/agent-1.meta.json"
    )));
    assert!(!is_transcript(Path::new("/p/-demo/notes.txt")));
}
