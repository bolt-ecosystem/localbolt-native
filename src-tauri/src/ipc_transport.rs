//! Cross-platform IPC stream abstraction.
//!
//! On Unix: wraps `UnixStream` for domain socket IPC.
//! On Windows: wraps `File` for named pipe IPC (compile-validated).

use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

/// Cross-platform IPC stream for daemon communication.
pub enum IpcStream {
    #[cfg(unix)]
    Unix(std::os::unix::net::UnixStream),
    #[cfg(windows)]
    Pipe(std::fs::File),
}

impl IpcStream {
    /// Connect to a daemon IPC endpoint.
    ///
    /// Detects transport type from path format:
    /// - `\\.\pipe\...` -> Windows named pipe (via File::open)
    /// - Everything else -> Unix domain socket
    pub fn connect(path: &Path) -> io::Result<Self> {
        let path_str = path.to_string_lossy();

        #[cfg(windows)]
        {
            if crate::platform::is_windows_pipe_path(&path_str) {
                let file = std::fs::OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(path)?;
                return Ok(Self::Pipe(file));
            }
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("non-pipe IPC path on Windows: {path_str}"),
            ))
        }

        #[cfg(unix)]
        {
            let _ = path_str;
            let stream = std::os::unix::net::UnixStream::connect(path)?;
            Ok(Self::Unix(stream))
        }

        #[cfg(all(not(unix), not(windows)))]
        {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unsupported IPC path: {path_str}"),
            ))
        }
    }

    /// Clone this stream (for separating reader/writer).
    pub fn try_clone(&self) -> io::Result<Self> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => Ok(Self::Unix(s.try_clone()?)),
            #[cfg(windows)]
            Self::Pipe(f) => Ok(Self::Pipe(f.try_clone()?)),
        }
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.set_read_timeout(timeout),
            #[cfg(windows)]
            Self::Pipe(_) => {
                // Named pipe client read timeout not directly configurable via File.
                let _ = timeout;
                Ok(())
            }
        }
    }

    /// Set write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.set_write_timeout(timeout),
            #[cfg(windows)]
            Self::Pipe(_) => {
                let _ = timeout;
                Ok(())
            }
        }
    }

    /// Quick probe: can we connect to this path?
    pub fn probe(path: &Path) -> bool {
        match Self::connect(path) {
            Ok(stream) => {
                let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
                drop(stream);
                true
            }
            Err(_) => false,
        }
    }
}

impl Read for IpcStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.read(buf),
            #[cfg(windows)]
            Self::Pipe(f) => f.read(buf),
        }
    }
}

impl Write for IpcStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.write(buf),
            #[cfg(windows)]
            Self::Pipe(f) => f.write(buf),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            #[cfg(unix)]
            Self::Unix(s) => s.flush(),
            #[cfg(windows)]
            Self::Pipe(f) => f.flush(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_fails_on_nonexistent() {
        let r = IpcStream::connect(Path::new("/tmp/bolt-nonexistent-transport-test.sock"));
        assert!(r.is_err());
    }

    #[test]
    fn probe_returns_false_for_nonexistent() {
        assert!(!IpcStream::probe(Path::new(
            "/tmp/bolt-nonexistent-probe-transport.sock"
        )));
    }

    #[cfg(unix)]
    #[test]
    fn probe_returns_true_for_listening() {
        use std::os::unix::net::UnixListener;
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::path::PathBuf::from(format!("/tmp/bolt-test-transport-{id}.sock"));
        let _ = std::fs::remove_file(&path);
        let _listener = UnixListener::bind(&path).unwrap();
        assert!(IpcStream::probe(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn unix_stream_read_write_roundtrip() {
        use std::io::BufRead;
        use std::os::unix::net::UnixListener;
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::path::PathBuf::from(format!("/tmp/bolt-test-transport-rt-{id}.sock"));
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();

        let p2 = path.clone();
        let handle = std::thread::spawn(move || {
            let mut stream = IpcStream::connect(&p2).unwrap();
            stream.write_all(b"hello\n").unwrap();
            stream.flush().unwrap();
        });

        let (accepted, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(accepted);
        let mut buf = String::new();
        reader.read_line(&mut buf).unwrap();
        assert_eq!(buf.trim(), "hello");

        handle.join().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}
