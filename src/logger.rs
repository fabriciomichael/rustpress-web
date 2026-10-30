//! Registo de eventos e diagnóstico do RustPress.
//!
//! O módulo fornece [`Logger`] e [`Level`]. É possível configurar o nível
//! mínimo de uma instância independente ou o logger interno de [`crate::App`].
//!
//! # Exemplo
//!
//! ```
//! use rustpress::{Level, Logger};
//! let logger = Logger::new();
//! logger.set_level(Level::Debug);
//! logger.info("Servidor preparado");
//! logger.warn("Configuração opcional ausente");
//! ```

use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

/// Nível de severidade de uma mensagem de log.
///
/// O nível configurado funciona como limite mínimo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u8)]
pub enum Level {
    /// Mensagens detalhadas para diagnóstico durante o desenvolvimento.
    Debug = 0,
    /// Eventos informativos normais da aplicação.
    Info = 1,
    /// Situações inesperadas que não interrompem necessariamente a aplicação.
    Warn = 2,
    /// Falhas que exigem atenção ou tratamento.
    Error = 3,
}

/// Logger leve para diagnóstico de aplicações RustPress.
///
/// Clones partilham o nível de registo da instância original; cada chamada a
/// [`Logger::new`] cria uma configuração independente.
#[derive(Debug, Clone)]
pub struct Logger {
    min_level: Arc<AtomicU8>,
}

impl Logger {
    /// Cria um novo logger com nível mínimo INFO.
    pub fn new() -> Self {
        Self {
            min_level: Arc::new(AtomicU8::new(Level::Info as u8)),
        }
    }

    /// Define o nível mínimo de registo.
    pub fn set_level(&self, level: Level) {
        self.min_level.store(level as u8, Ordering::Relaxed);
    }

    /// Verifica se uma mensagem deve ser apresentada.
    fn should_log(&self, level: Level) -> bool {
        (level as u8) >= self.min_level.load(Ordering::Relaxed)
    }

    /// Regista uma mensagem.
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

        let reset = "\x1b[0m";

        let output = format!("[{timestamp}] {color}[{name}]{reset} {message}");

        match level {
            Level::Debug | Level::Info => println!("{output}"),
            Level::Warn | Level::Error => eprintln!("{output}"),
        }
    }

    /// Regista uma mensagem de diagnóstico.
    pub fn debug(&self, message: &str) {
        self.log(Level::Debug, message);
    }

    /// Regista uma informação.
    pub fn info(&self, message: &str) {
        self.log(Level::Info, message);
    }

    /// Regista um aviso.
    pub fn warn(&self, message: &str) {
        self.log(Level::Warn, message);
    }

    /// Regista um erro.
    pub fn error(&self, message: &str) {
        self.log(Level::Error, message);
    }
}

impl Default for Logger {
    fn default() -> Self {
        Self::new()
    }
}

/// Obtém a data e hora actual em UTC.
fn current_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    format_timestamp(timestamp)
}

/// Converte um timestamp Unix para YYYY-MM-DD HH:MM:SS.
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
