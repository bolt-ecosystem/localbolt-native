//! Daemon stderr capture and crash snapshot persistence.
//!
//! Maintains an in-memory ring buffer of daemon stderr lines and writes
//! crash snapshots to disk when the daemon enters degraded state.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Default ring buffer capacity (lines).
const DEFAULT_CAPACITY: usize = 1000;
/// Lines to include in a crash snapshot.
const CRASH_SNAPSHOT_LINES: usize = 200;

/// Thread-safe ring buffer for daemon stderr lines.
#[derive(Clone)]
pub struct StderrBuffer {
    inner: Arc<Mutex<VecDeque<String>>>,
    capacity: usize,
}

impl StderrBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::with_capacity(capacity))),
            capacity,
        }
    }

    pub fn with_default_capacity() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }

    /// Push a line into the ring buffer, evicting oldest if at capacity.
    pub fn push(&self, line: String) {
        let mut buf = self.inner.lock().unwrap();
        if buf.len() >= self.capacity {
            buf.pop_front();
        }
        buf.push_back(line);
    }

    /// Get all lines currently in the buffer.
    #[cfg(test)]
    pub fn lines(&self) -> Vec<String> {
        self.inner.lock().unwrap().iter().cloned().collect()
    }

    /// Get the last N lines for crash snapshot.
    pub fn last_n(&self, n: usize) -> Vec<String> {
        let buf = self.inner.lock().unwrap();
        let skip = buf.len().saturating_sub(n);
        buf.iter().skip(skip).cloned().collect()
    }

    /// Clear the buffer.
    #[cfg(test)]
    pub fn clear(&self) {
        self.inner.lock().unwrap().clear();
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().len()
    }
}

/// Write a crash snapshot to disk.
///
/// Returns the path written, or an error.
pub fn write_crash_snapshot(
    buffer: &StderrBuffer,
    log_dir: &std::path::Path,
    exit_code: Option<i32>,
    pid: Option<u32>,
    retry_count: u32,
) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(log_dir)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let filename = format!("daemon-crash-{timestamp}.log");
    let path = log_dir.join(&filename);

    let lines = buffer.last_n(CRASH_SNAPSHOT_LINES);
    let code_str = exit_code
        .map(|c| c.to_string())
        .unwrap_or_else(|| "unknown".to_string());
    let pid_str = pid
        .map(|p| p.to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let header = format!(
        "[DAEMON_CRASH] exit_code={code_str} pid={pid_str} retry={retry_count}/3\n\
         --- stderr snapshot ({} lines) ---\n",
        lines.len()
    );

    let content = format!("{}{}", header, lines.join("\n"));
    std::fs::write(&path, content)?;

    tracing::warn!("[WATCHDOG] crash snapshot written: {}", path.display());

    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ring_buffer_capacity_enforced() {
        let buf = StderrBuffer::new(3);
        buf.push("a".into());
        buf.push("b".into());
        buf.push("c".into());
        buf.push("d".into());
        assert_eq!(buf.len(), 3);
        assert_eq!(buf.lines(), vec!["b", "c", "d"]);
    }

    #[test]
    fn last_n_returns_tail() {
        let buf = StderrBuffer::new(10);
        for i in 0..5 {
            buf.push(format!("line-{i}"));
        }
        let last = buf.last_n(2);
        assert_eq!(last, vec!["line-3", "line-4"]);
    }

    #[test]
    fn last_n_returns_all_if_fewer() {
        let buf = StderrBuffer::new(10);
        buf.push("only".into());
        let last = buf.last_n(5);
        assert_eq!(last, vec!["only"]);
    }

    #[test]
    fn clear_empties_buffer() {
        let buf = StderrBuffer::with_default_capacity();
        buf.push("x".into());
        buf.clear();
        assert_eq!(buf.len(), 0);
    }

    #[test]
    fn crash_snapshot_writes_file() {
        let dir = std::env::temp_dir().join("bolt-test-crash-snapshot");
        let _ = std::fs::remove_dir_all(&dir);
        let buf = StderrBuffer::new(10);
        buf.push("error line 1".into());
        buf.push("error line 2".into());

        let path = write_crash_snapshot(&buf, &dir, Some(1), Some(1234), 2).unwrap();
        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("[DAEMON_CRASH]"));
        assert!(content.contains("exit_code=1"));
        assert!(content.contains("pid=1234"));
        assert!(content.contains("retry=2/3"));
        assert!(content.contains("error line 1"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
