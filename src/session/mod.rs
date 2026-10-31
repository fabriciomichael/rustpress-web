//! Gestão de sessões e cookies do RustPress.

/// Representação e leitura de cookies HTTP.
pub mod cookie;

/// Configurações das sessões e dos cookies.
pub mod config;

/// Armazenamento interno dos dados das sessões.
pub mod store;

/// Gestão dos dados individuais de uma sessão.
pub mod session;

/// Gestão centralizada do ciclo de vida das sessões.
pub mod manager;

pub use config::{SameSite, SessionConfig};
pub use cookie::{Cookie, CookieError};
pub use manager::SessionManager;
pub use session::Session;
pub use store::{MemoryStore, SessionStore};
