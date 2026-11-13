//! Where Claude Code keeps its user config: `CLAUDE_CONFIG_DIR`, else
//! `~/.claude` (Windows: `%USERPROFILE%\.claude`).

use std::path::PathBuf;

pub fn config_dir() -> Option<PathBuf> {
    let non_empty = |name: &str| std::env::var_os(name).filter(|v| !v.is_empty());
    if let Some(dir) = non_empty("CLAUDE_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    let home = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    Some(PathBuf::from(non_empty(home)?).join(".claude"))
}
