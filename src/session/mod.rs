//! Gestão de sessões e cookies do RustPress.

/// Representação e leitura de cookies HTTP.
pub mod cookie;

/// Configurações das sessões e dos cookies.
pub mod config;

/// Armazenamento interno dos dados das sessões.
pub mod store;

/// Gestão dos dados individuais de uma sessão.
///
/// Mantém o caminho público `session::session` por compatibilidade.
#[allow(clippy::module_inception)]
pub mod session;

/// Gestão centralizada do ciclo de vida das sessões.
pub mod manager;

/// Erros que podem ocorrer durante operações com sessões.
#[derive(Debug)]
pub enum SessionError {
    /// O mutex interno da sessão foi envenenado.
    LockPoisoned,

    /// Ocorreu um erro no armazenamento da sessão.
    Store(SessionStoreError),

    /// Ocorreu um erro relacionado com cookies.
    Cookie(CookieError),

    /// A configuração necessária para gerir a sessão não está disponível.
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
