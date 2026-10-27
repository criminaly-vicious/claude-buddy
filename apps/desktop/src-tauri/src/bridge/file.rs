//! `bridge.json`: written atomically, readable by the current user only,
//! removed when the bridge stops (docs/architecture.md#discovery-file-bridgejson).

use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use bridge_protocol::BridgeFile;

/// A fresh token: 32 bytes from the OS CSPRNG as lowercase hex.
pub fn new_token() -> io::Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| io::Error::other(e.to_string()))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// Deletes the file when dropped, so a clean shutdown leaves nothing behind.
#[derive(Debug)]
pub struct FileGuard {
    path: PathBuf,
}

impl Drop for FileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

/// Writes `contents` to `path` via a temporary file in the same folder and a
/// rename, so readers never see a partial file.
pub fn write(path: &Path, contents: &BridgeFile) -> io::Result<FileGuard> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::other("bridge file path has no parent"))?;
    fs::create_dir_all(dir)?;
    let tmp = dir.join(format!(".bridge.{}.tmp", std::process::id()));

    let result = (|| {
        let mut file = create_private(&tmp)?;
        file.write_all(&serde_json::to_vec(contents).map_err(io::Error::other)?)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result?;
    Ok(FileGuard {
        path: path.to_path_buf(),
    })
}

/// Creates `path` so that only the current user can read it, before any
/// secret is written to it.
fn create_private(path: &Path) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    #[cfg(unix)]
    {
        // `mode` only applies when the file is new.
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    #[cfg(windows)]
    super::win_acl::restrict_to_current_user(path)?;
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("claude-buddy-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    fn sample() -> BridgeFile {
        BridgeFile {
            protocol: 1,
            port: 51234,
            token: new_token().unwrap(),
            pid: 1,
            started_at: 2,
        }
    }

    #[test]
    fn tokens_are_64_hex_chars_and_differ() {
        let a = new_token().unwrap();
        let b = new_token().unwrap();
        assert!(bridge_protocol::is_valid_token(&a));
        assert_ne!(a, b);
    }

    #[test]
    fn writes_readable_json_and_removes_it_on_drop() {
        let dir = temp_dir("write");
        let path = dir.join("nested").join("bridge.json");
        let contents = sample();

        let guard = write(&path, &contents).unwrap();
        let read: BridgeFile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(read, contents);
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers, ["bridge.json"], "no temporary file left");

        drop(guard);
        assert!(!path.exists());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn replaces_a_stale_file() {
        let dir = temp_dir("stale");
        let path = dir.join("bridge.json");
        fs::create_dir_all(&dir).unwrap();
        fs::write(&path, "stale").unwrap();

        let contents = sample();
        let _guard = write(&path, &contents).unwrap();
        let read: BridgeFile = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(read, contents);
        let _ = fs::remove_dir_all(dir);
    }

    #[cfg(unix)]
    #[test]
    fn file_is_private_to_the_user() {
        use std::os::unix::fs::PermissionsExt;
        let dir = temp_dir("mode");
        let path = dir.join("bridge.json");
        let _guard = write(&path, &sample()).unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let _ = fs::remove_dir_all(dir);
    }
}
