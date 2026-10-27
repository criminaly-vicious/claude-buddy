//! The tray's "Install / repair hooks" and "Uninstall hooks" actions: dry
//! run, confirmation with a summary, apply, then (for install) a check that
//! the bridge answers. The confirmation is injected so the whole flow is
//! tested without dialogs.
//!
//! The summary counts lines and names the file; it never shows the
//! settings' contents, which can hold secrets (`env`).

use std::path::{Path, PathBuf};

use super::health::{self, HealthError};
use crate::installer::{HookInstaller, InstallError, Plan, EVENTS};

/// What to tell the person after an action.
#[derive(Debug, PartialEq, Eq)]
pub struct Message {
    pub text: String,
    /// Something went wrong or needs their attention.
    pub warning: bool,
}

impl Message {
    fn info(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            warning: false,
        }
    }

    fn warning(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            warning: true,
        }
    }
}

pub struct HookActions {
    pub settings: PathBuf,
    /// `buddy-hook(.exe)`, expected next to the app.
    pub hook: PathBuf,
    /// `bridge.json`, for the health check.
    pub bridge_file: PathBuf,
}

impl HookActions {
    /// Install or repair. `confirm` gets the summary and says whether to go on;
    /// `None` when the person cancelled.
    pub fn install(&self, confirm: impl FnOnce(&str) -> bool) -> Option<Message> {
        if !self.hook.is_file() {
            return Some(Message::warning(format!(
                "buddy-hook was not found next to Claude Buddy:\n{}\n\nReinstall Claude Buddy, then try again.",
                self.hook.display()
            )));
        }
        let installer = HookInstaller::new(&self.settings, &self.hook);
        let plan = match installer.plan_install() {
            Ok(plan) => plan,
            Err(error) => return Some(read_failed(&self.settings, &error)),
        };
        let mut text = if plan.changes_anything() {
            if !confirm(&install_summary(&plan)) {
                return None;
            }
            match installer.install() {
                Ok(applied) => {
                    let mut text = format!("Hooks installed in\n{}", self.settings.display());
                    if let Some(backup) = applied.backup {
                        text +=
                            &format!("\n\nThe previous file was saved as\n{}", backup.display());
                    }
                    text
                }
                Err(error) => return Some(write_failed(&self.settings, &error)),
            }
        } else {
            "The hooks are already installed and up to date.".to_string()
        };
        let health = health::check(&self.bridge_file);
        text += &format!("\n\n{}", health_line(&health));
        Some(if health.is_ok() {
            Message::info(text)
        } else {
            Message::warning(text)
        })
    }

    /// Remove our hooks. `None` when the person cancelled.
    pub fn uninstall(&self, confirm: impl FnOnce(&str) -> bool) -> Option<Message> {
        let installer = HookInstaller::new(&self.settings, &self.hook);
        let plan = match installer.plan_uninstall() {
            Ok(plan) => plan,
            Err(error) => return Some(read_failed(&self.settings, &error)),
        };
        if !plan.changes_anything() {
            return Some(Message::info("No Claude Buddy hooks are installed."));
        }
        let (_, removed) = line_counts(&plan);
        let summary = format!(
            "Claude Buddy will remove its hooks from\n{}\n\n{removed} lines removed, nothing else changes. A backup of the current file is kept.\n\nBuddies will then only appear through the approximate transcript fallback.",
            plan.settings_path.display()
        );
        if !confirm(&summary) {
            return None;
        }
        Some(match installer.uninstall() {
            Ok(_) => Message::info(format!("Hooks removed from\n{}", self.settings.display())),
            Err(error) => write_failed(&self.settings, &error),
        })
    }
}

/// The confirmation text for an install.
pub fn install_summary(plan: &Plan) -> String {
    let (added, removed) = line_counts(plan);
    let file = if plan.before.is_empty() {
        "The file does not exist yet and will be created.".to_string()
    } else {
        format!("{added} lines added, {removed} removed. A backup of the current file is kept.")
    };
    format!(
        "Claude Buddy will add its hook to {} Claude Code events in\n{}\n\n{file}\n\nThe hook only reports what each session is doing to Claude Buddy on this computer. Sessions started from now on will show a buddy; running ones may need a restart.",
        EVENTS.len(),
        plan.settings_path.display()
    )
}

/// Lines added and removed by `plan`.
fn line_counts(plan: &Plan) -> (usize, usize) {
    plan.diff().lines().fold((0, 0), |(a, r), line| {
        if line.starts_with("+ ") {
            (a + 1, r)
        } else if line.starts_with("- ") {
            (a, r + 1)
        } else {
            (a, r)
        }
    })
}

fn health_line(health: &Result<(), HealthError>) -> String {
    match health {
        Ok(()) => "Bridge check: OK, sessions can reach Claude Buddy.".to_string(),
        Err(error) => format!("Bridge check failed: {error}. Buddies will not appear until Claude Buddy is restarted."),
    }
}

fn read_failed(settings: &Path, error: &InstallError) -> Message {
    Message::warning(format!(
        "Could not read\n{}\n\n{error}\n\nNothing was changed.",
        settings.display()
    ))
}

fn write_failed(settings: &Path, error: &InstallError) -> Message {
    Message::warning(format!(
        "Could not update\n{}\n\n{error}",
        settings.display()
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::bridge::Bridge;
    use crate::registry::Registry;

    struct Fixture {
        dir: PathBuf,
        actions: HookActions,
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn fixture(name: &str) -> Fixture {
        let dir =
            std::env::temp_dir().join(format!("claude-buddy-tray-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let hook = dir.join("buddy-hook.exe");
        std::fs::write(&hook, b"").unwrap();
        Fixture {
            actions: HookActions {
                settings: dir.join("settings.json"),
                hook,
                bridge_file: dir.join("bridge.json"),
            },
            dir,
        }
    }

    fn yes(_: &str) -> bool {
        true
    }

    #[test]
    fn install_confirms_then_writes_and_checks_the_bridge() {
        let f = fixture("install");
        std::fs::write(
            &f.actions.settings,
            "{\n  \"env\": { \"SECRET\": \"hunter2\" }\n}\n",
        )
        .unwrap();
        let _bridge = Bridge::start_at(
            &f.actions.bridge_file,
            Arc::new(Mutex::new(Registry::default())),
            |_| {},
        )
        .unwrap();
        let mut shown = String::new();
        let message = f
            .actions
            .install(|summary| {
                shown = summary.to_string();
                true
            })
            .unwrap();
        assert!(shown.contains("9 Claude Code events"), "{shown}");
        assert!(shown.contains("backup"), "{shown}");
        assert!(
            !shown.contains("hunter2"),
            "the summary never shows settings contents"
        );
        assert!(!message.warning, "{}", message.text);
        assert!(message.text.contains("Bridge check: OK"));
        assert!(message.text.contains("claude-buddy-backup"));
        let settings = std::fs::read_to_string(&f.actions.settings).unwrap();
        assert!(settings.contains("buddy-hook.exe") && settings.contains("hunter2"));

        // Second time: nothing to confirm.
        let again = f
            .actions
            .install(|_| panic!("no confirmation needed"))
            .unwrap();
        assert!(again.text.contains("already installed"));
    }

    #[test]
    fn cancelling_changes_nothing() {
        let f = fixture("cancel");
        assert_eq!(f.actions.install(|_| false), None);
        assert!(!f.actions.settings.exists());
    }

    #[test]
    fn a_new_settings_file_is_announced() {
        let f = fixture("new");
        let mut shown = String::new();
        f.actions.install(|s| {
            shown = s.to_string();
            false
        });
        assert!(shown.contains("will be created"), "{shown}");
    }

    #[test]
    fn missing_bridge_is_a_warning_but_hooks_are_installed() {
        let f = fixture("nobridge");
        let message = f.actions.install(yes).unwrap();
        assert!(message.warning);
        assert!(message.text.contains("Hooks installed"));
        assert!(message.text.contains("Bridge check failed"));
    }

    #[test]
    fn missing_hook_binary_stops_early() {
        let f = fixture("nohook");
        std::fs::remove_file(&f.actions.hook).unwrap();
        let message = f.actions.install(|_| panic!("must not ask")).unwrap();
        assert!(message.warning && message.text.contains("not found"));
        assert!(!f.actions.settings.exists());
    }

    #[test]
    fn invalid_settings_are_left_alone() {
        let f = fixture("invalid");
        std::fs::write(&f.actions.settings, "{ not json").unwrap();
        let message = f.actions.install(|_| panic!("must not ask")).unwrap();
        assert!(message.warning && message.text.contains("Nothing was changed"));
        assert_eq!(
            std::fs::read_to_string(&f.actions.settings).unwrap(),
            "{ not json"
        );
    }

    #[test]
    fn uninstall_after_install_and_when_nothing_is_there() {
        let f = fixture("uninstall");
        let nothing = f
            .actions
            .uninstall(|_| panic!("nothing to confirm"))
            .unwrap();
        assert!(nothing.text.contains("No Claude Buddy hooks"));
        f.actions.install(yes).unwrap();
        assert_eq!(f.actions.uninstall(|_| false), None);
        assert!(std::fs::read_to_string(&f.actions.settings)
            .unwrap()
            .contains("buddy-hook"));
        let removed = f.actions.uninstall(yes).unwrap();
        assert!(!removed.warning && removed.text.contains("Hooks removed"));
        assert!(!std::fs::read_to_string(&f.actions.settings)
            .unwrap()
            .contains("buddy-hook"));
    }
}
