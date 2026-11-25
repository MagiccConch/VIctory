//! One tracing setup for every binary.
//!
//! Filter with `VICTORY_LOG` (falls back to `RUST_LOG`), e.g.
//! `VICTORY_LOG=victory_loader=trace,info`. Debug and trace output exist
//! only in builds with the `diagnostics` feature; a plain release build
//! logs info and above whatever the filter asks for.
//!
//! Set `VICTORY_LOG_FILE=<path>` to also append every record to a file, so
//! a run's console output survives the window closing.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

/// The most verbose level this build can log.
const CAP: LevelFilter = if cfg!(feature = "diagnostics") {
    LevelFilter::TRACE
} else {
    LevelFilter::INFO
};

/// Appends to the log file. Shared handle behind a lock: a record's bytes
/// are written whole (and flushed), from whichever thread logs them.
struct FileLog(Arc<Mutex<std::fs::File>>);

impl std::io::Write for FileLog {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut file = self.0.lock().unwrap_or_else(|e| e.into_inner());
        file.write_all(buf)?;
        file.flush()?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub fn init(default_level: &str) {
    let filter = std::env::var("VICTORY_LOG")
        .or_else(|_| std::env::var("RUST_LOG"))
        .ok();
    let file = std::env::var_os("VICTORY_LOG_FILE").map(PathBuf::from);
    init_with(default_level, filter, file);
}

/// The env-less core of [`init`]: the filter from `VICTORY_LOG` (else the
/// default), and the log file path, if any.
fn init_with(default_level: &str, filter: Option<String>, file: Option<PathBuf>) {
    let filter = filter
        .and_then(|v| EnvFilter::try_new(v).ok())
        .unwrap_or_else(|| EnvFilter::new(default_level));
    let (log, file_error) = match file {
        Some(path) => match std::fs::OpenOptions::new().create(true).append(true).open(&path) {
            Ok(f) => (Some(Arc::new(Mutex::new(f))), None),
            Err(e) => (None, Some((path, e))),
        },
        None => (None, None),
    };
    let file = log.map(|log| {
        tracing_subscriber::fmt::layer()
            .with_target(true)
            .with_writer(move || FileLog(Arc::clone(&log)))
    });
    let _ = tracing_subscriber::registry()
        .with(CAP)
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(true)
                .with_writer(std::io::stderr),
        )
        .with(file)
        .try_init();
    if let Some((path, e)) = file_error {
        // The subscriber is live: this is seen on the console.
        tracing::warn!(error = %e, path = %path.display(), "no log file; logging to the console only");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// The file writer appends every write and keeps what earlier runs
    /// wrote (the file is opened for appending, never truncated).
    #[test]
    fn file_log_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.log");
        std::fs::write(&path, b"kept\n").unwrap();
        let log = Arc::new(Mutex::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .unwrap(),
        ));
        FileLog(Arc::clone(&log)).write_all(b"first\n").unwrap();
        // A later writer (another run, another thread) appends, not replaces.
        FileLog(log).write_all(b"second\n").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "kept\nfirst\nsecond\n");
    }

    /// An unusable log file path is no error: `init_with` goes on with the
    /// console alone. The parent directories are missing, so the open fails
    /// without creating anything.
    #[test]
    fn file_log_unusable() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("no/such/dir/run.log");
        init_with("info", None, Some(path));
    }
}
