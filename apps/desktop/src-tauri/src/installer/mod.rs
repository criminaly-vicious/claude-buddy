//! Installs and removes Claude Buddy's hooks in Claude Code's `settings.json`
//! (docs/session-detection.md#installer-output). The user's config is never
//! lost: every write is preceded by a timestamped backup, keys and their
//! order are preserved, other hooks are left alone, writes are atomic, and an
//! unreadable file is refused rather than overwritten.

mod diff;

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Map, Value};

/// Hook events the bridge maps (docs/architecture.md#server-event--registry).
/// `SubagentStart`/`SubagentStop` are left out: the MVP ignores them, so
/// registering them would only spawn processes for nothing.
pub const EVENTS: &[&str] = &[
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "PermissionRequest",
    "Stop",
    "SessionEnd",
];

/// File names that mark a hook command as ours, whatever folder it lives in.
const HOOK_FILE_NAMES: &[&str] = &["buddy-hook.exe", "buddy-hook"];

/// `timeout` written on each hook, in seconds. Claude Code does not enforce
/// it for async hooks; `buddy-hook` has its own much shorter limit.
const HOOK_TIMEOUT_SECS: u64 = 5;

/// Backups kept next to `settings.json`; older ones are deleted.
const MAX_BACKUPS: usize = 5;

/// `<config>/settings.json`, see [`crate::claude_dir::config_dir`].
pub fn default_settings_path() -> Option<PathBuf> {
    Some(crate::claude_dir::config_dir()?.join("settings.json"))
}

#[derive(Debug)]
pub enum InstallError {
    Io(io::Error),
    /// The file is not valid JSON. Nothing was written.
    InvalidJson {
        line: usize,
        column: usize,
    },
    /// Valid JSON, but not the shape Claude Code uses (e.g. `hooks` is not an
    /// object). Nothing was written.
    UnexpectedShape(&'static str),
}

impl fmt::Display for InstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::InvalidJson { line, column } => write!(
                f,
                "settings.json is not valid JSON (line {line}, column {column}); fix it first, nothing was changed"
            ),
            Self::UnexpectedShape(what) => {
                write!(f, "settings.json has an unexpected shape: {what}; nothing was changed")
            }
        }
    }
}

impl std::error::Error for InstallError {}

impl From<io::Error> for InstallError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

/// What an install or uninstall would do, without doing it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plan {
    pub settings_path: PathBuf,
    /// Current file contents (empty if the file does not exist).
    pub before: String,
    /// Contents after the change.
    pub after: String,
}

impl Plan {
    pub fn changes_anything(&self) -> bool {
        self.before != self.after
    }

    /// Line diff of `before` → `after`, `-`/`+` prefixed, for a preview.
    pub fn diff(&self) -> String {
        diff::lines(&self.before, &self.after)
    }
}

/// Result of applying a [`Plan`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Applied {
    /// Copy of the previous file, if there was one and it changed.
    pub backup: Option<PathBuf>,
    pub changed: bool,
}

pub struct HookInstaller {
    settings_path: PathBuf,
    hook_command: PathBuf,
}

impl HookInstaller {
    /// `hook_command` is the absolute path of `buddy-hook(.exe)` to register.
    pub fn new(settings_path: impl Into<PathBuf>, hook_command: impl Into<PathBuf>) -> Self {
        Self {
            settings_path: settings_path.into(),
            hook_command: hook_command.into(),
        }
    }

    /// Whether every event has exactly our current hook.
    pub fn is_installed(&self) -> Result<bool, InstallError> {
        let plan = self.plan_install()?;
        Ok(!plan.changes_anything() && !plan.before.is_empty())
    }

    pub fn plan_install(&self) -> Result<Plan, InstallError> {
        self.plan(install)
    }

    pub fn plan_uninstall(&self) -> Result<Plan, InstallError> {
        self.plan(|root, _| {
            uninstall(root);
            Ok(())
        })
    }

    pub fn install(&self) -> Result<Applied, InstallError> {
        self.apply(&self.plan_install()?)
    }

    pub fn uninstall(&self) -> Result<Applied, InstallError> {
        self.apply(&self.plan_uninstall()?)
    }

    fn plan(
        &self,
        change: impl FnOnce(&mut Map<String, Value>, &str) -> Result<(), InstallError>,
    ) -> Result<Plan, InstallError> {
        let before = match fs::read_to_string(&self.settings_path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => return Err(error.into()),
        };
        let mut root = parse(&before)?;
        let original = root.clone();
        change(&mut root, &self.hook_command.to_string_lossy())?;
        // Unchanged settings keep their exact bytes, formatting included.
        let after = if root == original {
            before.clone()
        } else {
            render(&root)
        };
        Ok(Plan {
            settings_path: self.settings_path.clone(),
            before,
            after,
        })
    }

    fn apply(&self, plan: &Plan) -> Result<Applied, InstallError> {
        if !plan.changes_anything() {
            return Ok(Applied {
                backup: None,
                changed: false,
            });
        }
        // Write through symlinks (dotfile setups), so the link survives.
        let target = resolve_symlinks(&self.settings_path)?;
        let backup = if plan.before.is_empty() {
            None
        } else {
            let backup = backup(&target)?;
            prune_backups(&target, MAX_BACKUPS);
            Some(backup)
        };
        write_atomically(&target, &plan.after)?;
        Ok(Applied {
            backup,
            changed: true,
        })
    }
}

/// Parses settings; an empty or blank file counts as `{}`.
fn parse(text: &str) -> Result<Map<String, Value>, InstallError> {
    if text.trim().is_empty() {
        return Ok(Map::new());
    }
    match serde_json::from_str::<Value>(text) {
        Ok(Value::Object(map)) => Ok(map),
        Ok(_) => Err(InstallError::UnexpectedShape(
            "the top level is not an object",
        )),
        Err(error) => Err(InstallError::InvalidJson {
            line: error.line(),
            column: error.column(),
        }),
    }
}

/// Two-space JSON, like Claude Code writes, with a trailing newline.
fn render(root: &Map<String, Value>) -> String {
    let mut text = serde_json::to_string_pretty(root).expect("a JSON map always serialises");
    text.push('\n');
    text
}

/// The entry Claude Code runs for `event` (exec form, async).
fn our_hook(command: &str, event: &str) -> Value {
    json!({
        "type": "command",
        "command": command,
        "args": [event],
        "async": true,
        "timeout": HOOK_TIMEOUT_SECS,
    })
}

fn is_ours(hook: &Value) -> bool {
    let Some(command) = hook.get("command").and_then(Value::as_str) else {
        return false;
    };
    let file_name = command
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    HOOK_FILE_NAMES.contains(&file_name.as_str())
}

/// Replaces any previous Claude Buddy hooks with the current ones.
fn install(root: &mut Map<String, Value>, command: &str) -> Result<(), InstallError> {
    if root.get("hooks").is_some_and(|h| !h.is_object()) {
        return Err(InstallError::UnexpectedShape("`hooks` is not an object"));
    }
    let wanted: Vec<(&str, Value)> = EVENTS.iter().map(|e| (*e, our_hook(command, e))).collect();
    if already_installed(root, &wanted) {
        return Ok(());
    }
    uninstall(root);
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .expect("checked above");
    for (event, hook) in wanted {
        let groups = hooks
            .entry(event)
            .or_insert_with(|| Value::Array(Vec::new()));
        let Some(groups) = groups.as_array_mut() else {
            return Err(InstallError::UnexpectedShape("a hook event is not a list"));
        };
        groups.push(json!({ "hooks": [hook] }));
    }
    Ok(())
}

/// True when each wanted event has exactly one hook of ours, equal to the
/// wanted one, and no other event has ours. Then install is a no-op and the
/// file keeps its exact bytes.
fn already_installed(root: &Map<String, Value>, wanted: &[(&str, Value)]) -> bool {
    let Some(hooks) = root.get("hooks").and_then(Value::as_object) else {
        return false;
    };
    let ours_in = |event: &str| -> Vec<&Value> {
        hooks
            .get(event)
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|group| group.get("hooks").and_then(Value::as_array))
            .flatten()
            .filter(|hook| is_ours(hook))
            .collect()
    };
    let wanted_ok = wanted.iter().all(|(event, hook)| ours_in(event) == [hook]);
    let no_extra = hooks
        .keys()
        .filter(|event| !EVENTS.contains(&event.as_str()))
        .all(|event| ours_in(event).is_empty());
    wanted_ok && no_extra
}

/// Removes every Claude Buddy hook. Groups, event lists and the `hooks`
/// object are removed only when our removal emptied them.
fn uninstall(root: &mut Map<String, Value>) {
    let Some(hooks) = root.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let mut emptied_events = Vec::new();
    for (event, groups) in hooks.iter_mut() {
        let Some(groups) = groups.as_array_mut() else {
            continue;
        };
        let before = groups.len();
        groups.retain_mut(|group| {
            let Some(list) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
                return true;
            };
            let had = list.len();
            list.retain(|hook| !is_ours(hook));
            // Keep groups that still hold someone else's hooks, or that we did not touch.
            !(list.is_empty() && had > 0)
        });
        if groups.is_empty() && before > 0 {
            emptied_events.push(event.clone());
        }
    }
    let removed_any = !emptied_events.is_empty();
    for event in emptied_events {
        hooks.shift_remove(&event);
    }
    if hooks.is_empty() && removed_any {
        root.shift_remove("hooks");
    }
}

/// The file `path` finally points to, following symlinks (also dangling
/// ones, so a link to a not-yet-created file is written through too).
fn resolve_symlinks(path: &Path) -> io::Result<PathBuf> {
    let mut current = path.to_path_buf();
    for _ in 0..40 {
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                let link = fs::read_link(&current)?;
                current = match current.parent() {
                    Some(dir) if link.is_relative() => dir.join(link),
                    _ => link,
                };
            }
            _ => return Ok(current),
        }
    }
    Err(io::Error::other("too many levels of symbolic links"))
}

/// Deletes all but the newest `keep` backups of `path`.
fn prune_backups(path: &Path, keep: usize) {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return;
    };
    let prefix = format!("{}.claude-buddy-backup-", name.to_string_lossy());
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut backups: Vec<(u128, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let millis = file_name.strip_prefix(&prefix)?.parse().ok()?;
            Some((millis, entry.path()))
        })
        .collect();
    backups.sort_unstable_by(|a, b| b.0.cmp(&a.0));
    for (_, old) in backups.into_iter().skip(keep) {
        let _ = fs::remove_file(old);
    }
}

/// Copies `path` to `<path>.claude-buddy-backup-<epoch ms>` next to it.
fn backup(path: &Path) -> io::Result<PathBuf> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".claude-buddy-backup-{millis}"));
    let target = path.with_file_name(name);
    fs::copy(path, &target)?;
    Ok(target)
}

/// Writes via a temporary file and a rename, keeping the original's
/// permissions on Unix.
fn write_atomically(path: &Path, contents: &str) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(format!(".claude-buddy-{}.tmp", std::process::id()));
    let tmp = path.with_file_name(tmp_name);
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        if let Ok(metadata) = fs::metadata(path) {
            fs::set_permissions(&tmp, metadata.permissions())?;
        }
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests;
