//! Reads only the new complete lines of a growing file.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

/// How much of an existing file is read when it is first seen, so that a
/// long-running session does not replay hours of history.
pub const INITIAL_TAIL_BYTES: u64 = 64 * 1024;

#[derive(Debug, Default)]
pub struct Tail {
    offset: u64,
    /// Bytes after the last newline, waiting for the rest of their line.
    partial: Vec<u8>,
    /// Drop everything up to and including the first newline: we started
    /// one byte before the cut, so this removes exactly the cut-off line (or
    /// just the newline, when the cut fell on a line start).
    skip_to_newline: bool,
}

impl Tail {
    /// Starts near the end of a file of `len` bytes.
    pub fn near_end(len: u64) -> Self {
        let offset = len.saturating_sub(INITIAL_TAIL_BYTES + 1);
        Self {
            offset,
            partial: Vec::new(),
            skip_to_newline: offset > 0,
        }
    }

    /// New complete lines since the last call. A file that shrank (truncated
    /// or replaced) is read again from the start.
    pub fn read_new(&mut self, path: &Path) -> io::Result<Vec<String>> {
        let mut file = File::open(path)?;
        let len = file.metadata()?.len();
        if len < self.offset {
            *self = Self::default();
        }
        if len == self.offset {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(len - self.offset).read_to_end(&mut bytes)?;
        self.offset += bytes.len() as u64;

        self.partial.extend_from_slice(&bytes);
        let Some(last_newline) = self.partial.iter().rposition(|&b| b == b'\n') else {
            return Ok(Vec::new());
        };
        let rest = self.partial.split_off(last_newline + 1);
        let mut complete = std::mem::replace(&mut self.partial, rest);
        if std::mem::take(&mut self.skip_to_newline) {
            let first_newline = complete
                .iter()
                .position(|&b| b == b'\n')
                .unwrap_or_default();
            complete.drain(..=first_newline);
        }

        let lines: Vec<String> = complete
            .split(|&b| b == b'\n')
            .filter(|l| !l.is_empty())
            .map(|l| {
                String::from_utf8_lossy(l)
                    .trim_end_matches('\r')
                    .to_string()
            })
            .collect();
        Ok(lines)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    fn temp_file(name: &str) -> std::path::PathBuf {
        let path = std::env::temp_dir().join(format!(
            "claude-buddy-tail-{name}-{}.jsonl",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        path
    }

    fn append(path: &Path, text: &str) {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        file.write_all(text.as_bytes()).unwrap();
    }

    #[test]
    fn returns_only_new_complete_lines() {
        let path = temp_file("new");
        let mut tail = Tail::default();
        append(&path, "a\nb\npar");
        assert_eq!(tail.read_new(&path).unwrap(), ["a", "b"]);
        assert!(tail.read_new(&path).unwrap().is_empty());
        append(&path, "tial\r\nc\n");
        assert_eq!(tail.read_new(&path).unwrap(), ["partial", "c"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn truncated_file_is_read_from_the_start() {
        let path = temp_file("truncate");
        let mut tail = Tail::default();
        append(&path, "old line one\nold line two\n");
        tail.read_new(&path).unwrap();
        std::fs::write(&path, "new\n").unwrap();
        assert_eq!(tail.read_new(&path).unwrap(), ["new"]);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn near_end_skips_history_and_the_cut_line() {
        let path = temp_file("near-end");
        let line = format!("{}\n", "x".repeat(1000));
        for _ in 0..100 {
            append(&path, &line);
        }
        append(&path, "last\n");
        let len = std::fs::metadata(&path).unwrap().len();
        let mut tail = Tail::near_end(len);
        let lines = tail.read_new(&path).unwrap();
        assert_eq!(lines.last().map(String::as_str), Some("last"));
        assert!(lines.len() < 70, "only the last 64 KiB");
        assert!(
            lines[..lines.len() - 1].iter().all(|l| l.len() == 1000),
            "no cut line"
        );
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn small_file_near_end_is_read_whole() {
        let path = temp_file("small");
        append(&path, "one\ntwo\n");
        let len = std::fs::metadata(&path).unwrap().len();
        assert_eq!(Tail::near_end(len).read_new(&path).unwrap(), ["one", "two"]);
        let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod boundary_tests {
    use super::*;

    /// Regression for #47: a cut exactly at a line start must not drop that line.
    #[test]
    fn cut_on_a_line_start_keeps_the_line() {
        let path = std::env::temp_dir().join(format!(
            "claude-buddy-tail-boundary-{}.jsonl",
            std::process::id()
        ));
        // A first line of 99 bytes + newline, then exactly 64 KiB of 1 KiB lines.
        let head = format!("{}\n", "h".repeat(99));
        let line = format!("{}\n", "x".repeat(1023));
        let body = line.repeat((INITIAL_TAIL_BYTES / 1024) as usize);
        std::fs::write(&path, format!("{head}{body}")).unwrap();
        let len = std::fs::metadata(&path).unwrap().len();
        assert_eq!(
            len - INITIAL_TAIL_BYTES,
            head.len() as u64,
            "cut falls on a line start"
        );

        let lines = Tail::near_end(len).read_new(&path).unwrap();
        assert_eq!(lines.len(), 64, "all 64 whole lines after the cut");
        assert!(lines.iter().all(|l| l.len() == 1023));
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn cut_mid_line_drops_only_the_partial_line() {
        let path = std::env::temp_dir().join(format!(
            "claude-buddy-tail-midline-{}.jsonl",
            std::process::id()
        ));
        let head = format!("{}\n", "h".repeat(500));
        let line = format!("{}\n", "x".repeat(1023));
        let body = line.repeat((INITIAL_TAIL_BYTES / 1024) as usize);
        // Shift by 10 bytes so the cut lands inside the first body line.
        std::fs::write(&path, format!("{head}{body}tail-line\n")).unwrap();
        let len = std::fs::metadata(&path).unwrap().len();

        let lines = Tail::near_end(len).read_new(&path).unwrap();
        assert_eq!(lines.last().map(String::as_str), Some("tail-line"));
        assert!(
            lines[..lines.len() - 1].iter().all(|l| l.len() == 1023),
            "no cut line"
        );
        let _ = std::fs::remove_file(path);
    }
}
