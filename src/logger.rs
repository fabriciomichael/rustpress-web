//! RustPress event logging and diagnostics.
//!
//! This module provides [`Logger`] and [`Level`]. You can configure the minimum
//! level of an independent instance or the internal logger of [`crate::App`].
//!
//! # Example
//!
//! ```
//! use rustpress_web::{Level, Logger};
//! let logger = Logger::new();
//! logger.set_level(Level::Debug);
//! logger.info("Server ready");
//! logger.warn("Optional configuration missing");
//! ```

use is_terminal::IsTerminal;
use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

/// Severity level of a log message.
///
/// The configured level acts as the minimum threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    /// Detailed messages for diagnostics during development.
    Debug = 0,
    /// Normal informational application events.
    Info = 1,
    /// Unexpected situations that do not necessarily interrupt the application.
    Warn = 2,
    /// Failures that require attention or handling.
    Error = 3,
}

/// Lightweight logger for RustPress application diagnostics.
///
/// Clones share the logging level of the original instance; each call to
/// [`Logger::new`] creates an independent configuration.
#[derive(Debug, Clone)]
pub struct Logger {
    min_level: Arc<AtomicU8>,
}

impl Logger {
    /// Creates a new logger with an INFO minimum level.
    pub fn new() -> Self {
        Self {
            min_level: Arc::new(AtomicU8::new(Level::Info as u8)),
        }
    }

    /// Sets the minimum logging level.
    pub fn set_level(&self, level: Level) {
        self.min_level.store(level as u8, Ordering::Relaxed);
    }

    /// Checks whether a message should be displayed.
    fn should_log(&self, level: Level) -> bool {
        (level as u8) >= self.min_level.load(Ordering::Relaxed)
    }

    /// Logs a message.
    fn log(&self, level: Level, message: &str) {
        if !self.should_log(level) {
            return;
        }

        let timestamp = current_timestamp();

        let (name, color) = match level {
            Level::Debug => ("DEBUG", "\x1b[36m"),
            Level::Info => ("INFO", "\x1b[32m"),
            Level::Warn => ("WARN", "\x1b[33m"),
            Level::Error => ("ERROR", "\x1b[31m"),
        };

        let output = match level {
            Level::Debug | Level::Info if std::io::stdout().is_terminal() => {
                format!("[{timestamp}] {color}[{name}]\x1b[0m {message}")
            }
            Level::Warn | Level::Error if std::io::stderr().is_terminal() => {
                format!("[{timestamp}] {color}[{name}]\x1b[0m {message}")
            }
            _ => format!("[{timestamp}] [{name}] {message}"),
        };

        match level {
            Level::Debug | Level::Info => println!("{output}"),
            Level::Warn | Level::Error => eprintln!("{output}"),
        }
    }

    /// Logs a diagnostic message.
    pub fn debug(&self, message: &str) {
        self.log(Level::Debug, message);
    }

    /// Logs an informational message.
    pub fn info(&self, message: &str) {
        self.log(Level::Info, message);
    }

    /// Logs a warning.
    pub fn warn(&self, message: &str) {
        self.log(Level::Warn, message);
    }

    /// Logs an error.
    pub fn error(&self, message: &str) {
        self.log(Level::Error, message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

/// Gets the current date and time in UTC.
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    format_timestamp(timestamp)
}

/// Converts a Unix timestamp to YYYY-MM-DD HH:MM:SS.
fn format_timestamp(timestamp: u64) -> String {
    let days = (timestamp / 86_400) as i64;
    let seconds = timestamp % 86_400;

    let hour = seconds / 3_600;
    let minute = (seconds % 3_600) / 60;
    let second = seconds % 60;

    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;

    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };

    if month <= 2 {
        year += 1;
    }

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

#[cfg(test)]
mod tests {
    use super::{Level, Logger, format_timestamp};

    #[test]
    fn formats_unix_epoch_correctly() {
        assert_eq!(format_timestamp(0), "1970-01-01 00:00:00");
    }

    #[test]
    fn formats_one_day_after_unix_epoch() {
        assert_eq!(format_timestamp(86_400), "1970-01-02 00:00:00");
    }

    #[test]
    fn formats_timestamp_with_time() {
        assert_eq!(format_timestamp(3_661), "1970-01-01 01:01:01");
    }

    #[test]
    fn loggers_have_independent_levels() {
        let logger1 = Logger::new();
        let logger2 = Logger::new();

        logger1.set_level(Level::Debug);
        logger2.set_level(Level::Error);

        assert!(logger1.should_log(Level::Debug));
        assert!(!logger2.should_log(Level::Debug));

        assert!(logger1.should_log(Level::Info));
        assert!(!logger2.should_log(Level::Info));

        assert!(logger2.should_log(Level::Error));
    }

    #[test]
    fn cloned_logger_shares_configuration() {
        let logger1 = Logger::new();
        let logger2 = logger1.clone();

        logger1.set_level(Level::Error);

        assert!(!logger2.should_log(Level::Info));
        assert!(logger2.should_log(Level::Error));
    }
}
