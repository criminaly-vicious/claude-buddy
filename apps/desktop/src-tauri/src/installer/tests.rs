use super::*;

const HOOK: &str = r"C:\Users\me\AppData\Local\Claude Buddy\buddy-hook.exe";

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../../../tests/fixtures/settings/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

/// A temporary settings file, optionally seeded with `contents`.
fn settings(name: &str, contents: Option<&str>) -> (HookInstaller, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "claude-buddy-installer-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.json");
    if let Some(contents) = contents {
        fs::write(&path, contents).unwrap();
    }
    (HookInstaller::new(&path, HOOK), dir)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn ours(settings: &Value, event: &str) -> Vec<Value> {
    settings["hooks"][event]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|group| group["hooks"].as_array().cloned().unwrap_or_default())
        .filter(is_ours)
        .collect()
}

fn backups(dir: &Path) -> Vec<PathBuf> {
    fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.to_string_lossy().contains(".claude-buddy-backup-"))
        .collect()
}

#[test]
fn installs_every_event_in_exec_async_form() {
    let (installer, dir) = settings("fresh", Some(&fixture("no-hooks.json")));
    let applied = installer.install().unwrap();
    assert!(applied.changed);

    let json = read_json(&dir.join("settings.json"));
    for event in EVENTS {
        assert_eq!(
            ours(&json, event),
            [json!({
                "type": "command",
                "command": HOOK,
                "args": [event],
                "async": true,
                "timeout": 5
            })],
            "{event}"
        );
    }
    assert!(json["hooks"].get("SubagentStart").is_none());
    assert!(installer.is_installed().unwrap());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn keeps_other_keys_and_their_order() {
    let (installer, dir) = settings("order", Some(&fixture("no-hooks.json")));
    installer.install().unwrap();
    let json = read_json(&dir.join("settings.json"));
    let keys: Vec<_> = json.as_object().unwrap().keys().cloned().collect();
    assert_eq!(
        keys,
        [
            "$schema",
            "model",
            "permissions",
            "env",
            "statusLine",
            "hooks"
        ]
    );
    let original: Value = serde_json::from_str(&fixture("no-hooks.json")).unwrap();
    for key in ["$schema", "model", "permissions", "env", "statusLine"] {
        assert_eq!(json[key], original[key], "{key}");
    }
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn leaves_third_party_hooks_alone() {
    let (installer, dir) = settings("third-party", Some(&fixture("third-party-hooks.json")));
    installer.install().unwrap();
    let json = read_json(&dir.join("settings.json"));

    let pre = json["hooks"]["PreToolUse"].as_array().unwrap();
    assert_eq!(pre[0]["matcher"], "Bash");
    assert_eq!(
        pre[0]["hooks"][0]["command"],
        "/home/user/bin/audit-bash.sh"
    );
    assert_eq!(pre.len(), 2, "ours is a separate group at the end");
    assert_eq!(
        json["hooks"]["Stop"][0]["hooks"][0]["command"],
        "notify-send 'Claude finished'"
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn install_then_uninstall_restores_the_file_byte_for_byte() {
    for name in ["no-hooks.json", "third-party-hooks.json"] {
        let original = fixture(name);
        let (installer, dir) = settings(&format!("roundtrip-{name}"), Some(&original));
        installer.install().unwrap();
        assert_ne!(
            fs::read_to_string(dir.join("settings.json")).unwrap(),
            original
        );
        installer.uninstall().unwrap();
        assert_eq!(
            fs::read_to_string(dir.join("settings.json")).unwrap(),
            original,
            "{name}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn install_is_idempotent_and_keeps_bytes_when_nothing_changes() {
    let (installer, dir) = settings("idempotent", Some(&fixture("third-party-hooks.json")));
    installer.install().unwrap();
    let once = fs::read_to_string(dir.join("settings.json")).unwrap();
    let again = installer.install().unwrap();
    assert!(!again.changed);
    assert_eq!(again.backup, None, "no backup when nothing is written");
    assert_eq!(fs::read_to_string(dir.join("settings.json")).unwrap(), once);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn upgrades_an_old_install_and_keeps_shared_groups() {
    let (installer, dir) = settings("old", Some(&fixture("old-buddy.json")));
    assert!(!installer.is_installed().unwrap());
    installer.install().unwrap();
    let json = read_json(&dir.join("settings.json"));

    // Old entry replaced by the current one, at the new path.
    assert_eq!(ours(&json, "PreToolUse").len(), 1);
    assert_eq!(ours(&json, "PreToolUse")[0]["command"], HOOK);
    // The group we shared keeps the other tool's hook.
    assert_eq!(
        json["hooks"]["PreToolUse"][0]["hooks"],
        json!([{"type": "command", "command": "/home/user/bin/audit-bash.sh"}])
    );
    // An event we no longer register is cleaned up.
    assert!(json["hooks"].get("SubagentStart").is_none());
    assert_eq!(json["theme"], "dark");
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn uninstall_removes_only_ours() {
    let (installer, dir) = settings("uninstall-old", Some(&fixture("old-buddy.json")));
    installer.uninstall().unwrap();
    let json = read_json(&dir.join("settings.json"));
    assert_eq!(
        json,
        json!({
            "hooks": {
                "PreToolUse": [
                    {"hooks": [{"type": "command", "command": "/home/user/bin/audit-bash.sh"}]}
                ]
            },
            "theme": "dark"
        })
    );
    let _ = fs::remove_dir_all(dir);
}

/// Uninstall cannot tell an empty container the user had from one an install
/// created, so it removes both. Empty hook lists mean the same to Claude Code
/// as missing ones, so the round trip is semantically identical.
#[test]
fn empty_containers_round_trip_semantically() {
    for (original, expected) in [
        ("{\n  \"hooks\": {}\n}\n", json!({})),
        (
            r#"{"hooks": {"Stop": [], "PreToolUse": [{"hooks": [{"type": "command", "command": "x"}]}]}}"#,
            json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "x"}]}]}}),
        ),
    ] {
        let (installer, dir) = settings("empty-containers", Some(original));
        installer.install().unwrap();
        installer.uninstall().unwrap();
        assert_eq!(
            read_json(&dir.join("settings.json")),
            expected,
            "{original}"
        );
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn uninstall_without_our_hooks_keeps_the_exact_bytes() {
    let original = "{\n  \"hooks\": {},\n    \"model\":   \"opus\"\n}";
    let (installer, dir) = settings("untouched", Some(original));
    assert!(!installer.uninstall().unwrap().changed);
    assert_eq!(
        fs::read_to_string(dir.join("settings.json")).unwrap(),
        original
    );
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn missing_file_is_created_and_uninstall_is_a_no_op() {
    let (installer, dir) = settings("missing", None);
    assert!(!installer.uninstall().unwrap().changed);
    assert!(!dir.join("settings.json").exists());

    let applied = installer.install().unwrap();
    assert_eq!(applied.backup, None, "nothing to back up");
    assert!(installer.is_installed().unwrap());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn blank_file_counts_as_empty_settings() {
    let (installer, dir) = settings("blank", Some("  \n"));
    installer.install().unwrap();
    assert!(installer.is_installed().unwrap());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn backs_up_before_every_write() {
    let original = fixture("third-party-hooks.json");
    let (installer, dir) = settings("backup", Some(&original));
    let applied = installer.install().unwrap();
    let backup = applied.backup.expect("a backup");
    assert_eq!(fs::read_to_string(&backup).unwrap(), original);
    assert_eq!(backups(&dir), [backup]);
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn invalid_json_is_refused_and_left_untouched() {
    let original = fixture("invalid.json");
    let (installer, dir) = settings("invalid", Some(&original));
    for result in [installer.install(), installer.uninstall()] {
        match result {
            Err(InstallError::InvalidJson { line, .. }) => assert_eq!(line, 4),
            other => panic!("expected InvalidJson, got {other:?}"),
        }
    }
    assert_eq!(
        fs::read_to_string(dir.join("settings.json")).unwrap(),
        original
    );
    assert!(backups(&dir).is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn unexpected_shapes_are_refused() {
    for (contents, why) in [
        ("[]", "the top level is not an object"),
        (r#"{"hooks": []}"#, "`hooks` is not an object"),
        (r#"{"hooks": {"Stop": {}}}"#, "a hook event is not a list"),
    ] {
        let (installer, dir) = settings("shape", Some(contents));
        match installer.install() {
            Err(InstallError::UnexpectedShape(what)) => assert_eq!(what, why),
            other => panic!("{contents}: expected UnexpectedShape, got {other:?}"),
        }
        assert_eq!(
            fs::read_to_string(dir.join("settings.json")).unwrap(),
            contents
        );
        let _ = fs::remove_dir_all(dir);
    }
}

#[test]
fn dry_run_writes_nothing_and_shows_the_diff() {
    let original = fixture("no-hooks.json");
    let (installer, dir) = settings("dry-run", Some(&original));
    let plan = installer.plan_install().unwrap();
    assert!(plan.changes_anything());
    assert_eq!(plan.before, original);
    let diff = plan.diff();
    assert!(diff.contains("+   \"hooks\": {"), "{diff}");
    assert!(
        diff.contains("  \"model\": \"opus\","),
        "unchanged lines are kept"
    );
    assert!(
        !diff.lines().any(|l| l.starts_with("- ")),
        "nothing removed"
    );

    assert_eq!(
        fs::read_to_string(dir.join("settings.json")).unwrap(),
        original
    );
    assert!(backups(&dir).is_empty());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn hooks_are_recognised_by_file_name_on_any_path() {
    for command in [
        r"C:\Program Files\Claude Buddy\buddy-hook.exe",
        r"C:\Users\me\AppData\Local\Claude Buddy\BUDDY-HOOK.EXE",
        "/home/me/.local/bin/buddy-hook",
    ] {
        assert!(is_ours(&json!({ "command": command })), "{command}");
    }
    for command in [
        "/usr/bin/other-hook",
        "buddy-hook-wrapper.sh",
        "notify buddy-hook",
    ] {
        assert!(!is_ours(&json!({ "command": command })), "{command}");
    }
}

#[cfg(unix)]
#[test]
fn writes_through_a_symlinked_settings_file() {
    let original = fixture("no-hooks.json");
    let (installer, dir) = settings("symlink", None);
    let dotfiles = dir.join("dotfiles");
    fs::create_dir_all(&dotfiles).unwrap();
    let real = dotfiles.join("settings.json");
    fs::write(&real, &original).unwrap();
    let link = dir.join("settings.json");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let applied = installer.install().unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(fs::read_to_string(&real)
        .unwrap()
        .contains("buddy-hook.exe"));
    let backup = applied.backup.unwrap();
    assert_eq!(
        backup.parent(),
        Some(dotfiles.as_path()),
        "backup next to the real file"
    );

    installer.uninstall().unwrap();
    assert!(fs::symlink_metadata(&link)
        .unwrap()
        .file_type()
        .is_symlink());
    assert_eq!(fs::read_to_string(&real).unwrap(), original);
    let _ = fs::remove_dir_all(dir);
}

#[cfg(unix)]
#[test]
fn writes_through_a_dangling_relative_symlink() {
    let (installer, dir) = settings("dangling", None);
    std::os::unix::fs::symlink("dotfiles/settings.json", dir.join("settings.json")).unwrap();
    fs::create_dir_all(dir.join("dotfiles")).unwrap();

    installer.install().unwrap();
    assert!(fs::symlink_metadata(dir.join("settings.json"))
        .unwrap()
        .file_type()
        .is_symlink());
    assert!(installer.is_installed().unwrap());
    assert!(dir.join("dotfiles/settings.json").is_file());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn keeps_only_the_newest_backups() {
    let (installer, dir) = settings("prune", Some(&fixture("no-hooks.json")));
    let path = dir.join("settings.json");
    for millis in [1, 2, 3, 4, 5, 6, 7] {
        fs::write(
            dir.join(format!("settings.json.claude-buddy-backup-{millis}")),
            "old",
        )
        .unwrap();
    }
    fs::write(
        dir.join("settings.json.claude-buddy-backup-notanumber"),
        "keep me",
    )
    .unwrap();

    installer.install().unwrap();
    let mut left: Vec<String> = backups(&dir)
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    left.sort();
    assert_eq!(left.len(), MAX_BACKUPS + 1, "{left:?}");
    assert!(left.contains(&"settings.json.claude-buddy-backup-notanumber".to_string()));
    for gone in [1, 2, 3] {
        assert!(!left.contains(&format!("settings.json.claude-buddy-backup-{gone}")));
    }
    assert!(path.is_file());
    let _ = fs::remove_dir_all(dir);
}

#[test]
fn uninstall_cli_removes_only_our_hooks() {
    let (installer, dir) = settings("cli", Some(&fixture("no-hooks.json")));
    installer.install().unwrap();
    let path = dir.join("settings.json");
    let mut json = read_json(&path);
    json["hooks"]["Stop"][0]["hooks"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type": "command", "command": "notify-send done"}));
    fs::write(&path, serde_json::to_string_pretty(&json).unwrap()).unwrap();

    assert_eq!(uninstall_at(&path), 0);
    let after = read_json(&path);
    for event in EVENTS {
        assert!(ours(&after, event).is_empty(), "{event}");
    }
    assert_eq!(
        after["hooks"]["Stop"][0]["hooks"],
        json!([{"type": "command", "command": "notify-send done"}])
    );
    assert_eq!(uninstall_at(&path), 0, "nothing left to remove");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn uninstall_cli_succeeds_without_a_settings_file() {
    let (_, dir) = settings("cli-missing", None);
    assert_eq!(uninstall_at(&dir.join("settings.json")), 0);
    assert!(!dir.join("settings.json").exists());
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn uninstall_cli_fails_on_invalid_json_and_leaves_it() {
    let (_, dir) = settings("cli-invalid", Some("{ not json"));
    let path = dir.join("settings.json");
    assert_eq!(uninstall_at(&path), 1);
    assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
    let _ = fs::remove_dir_all(&dir);
}
