//! RustPress session and cookie management.

/// HTTP cookie representation and parsing.
pub mod cookie;

/// Session and cookie configuration.
pub mod config;

/// Internal storage for session data.
pub mod store;

/// Management of individual session data.
///
/// Keeps the public `session::session` path for compatibility.
#[allow(clippy::module_inception)]
pub mod session;

/// Centralized session lifecycle management.
pub mod manager;

/// Errors that can occur during session operations.
#[derive(Debug)]
pub enum SessionError {
    /// The session's internal mutex was poisoned.
    LockPoisoned,

    /// An error occurred in session storage.
    Store(SessionStoreError),

    /// A cookie-related error occurred.
    Cookie(CookieError),

    /// The configuration required to manage the session is unavailable.
    Configuration,
}

impl std::fmt::Display for SessionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LockPoisoned => {
                write!(f, "O acesso à sessão está indisponível.")
            }
            Self::Store(error) => write!(f, "{error}"),
            Self::Cookie(error) => write!(f, "Erro de cookie: {error}"),
            Self::Configuration => write!(f, "Configuração de sessão não disponível"),
        }
    }
}

impl std::error::Error for SessionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::LockPoisoned | Self::Configuration => None,
            Self::Store(error) => Some(error),
            Self::Cookie(error) => Some(error),
        }
    }
}

impl From<CookieError> for SessionError {
    fn from(error: CookieError) -> Self {
        Self::Cookie(error)
    }
}

impl From<SessionStoreError> for SessionError {
    fn from(error: SessionStoreError) -> Self {
        Self::Store(error)
    }
}

pub use config::{SameSite, SessionConfig};
pub use cookie::{Cookie, CookieError};
pub use manager::SessionManager;
pub use session::Session;
pub use store::{FileStore, MemoryStore, SessionStore, SessionStoreError};
