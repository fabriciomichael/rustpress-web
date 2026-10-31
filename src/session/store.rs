//! Armazenamento interno dos dados das sessões.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

use serde_json::Value;

/// Interface para armazenamento dos dados das sessões.
pub trait SessionStore: Send + Sync {
    /// Obtém os dados de uma sessão pelo seu identificador.
    fn get(&self, session_id: &str) -> Option<Value>;

    /// Guarda ou actualiza os dados de uma sessão.
    fn set(&self, session_id: String, data: Value);

    /// Guarda uma sessão com um tempo de vida limitado.
    ///
    /// A implementação padrão mantém a compatibilidade com
    /// armazenamentos que ainda não suportam expiração.
    fn set_with_ttl(&self, session_id: String, data: Value, _ttl: Duration) {
        self.set(session_id, data);
    }

    /// Remove uma sessão do armazenamento.
    fn remove(&self, session_id: &str);

    /// Revoga uma sessão, impedindo gravações posteriores com o mesmo ID.
    ///
    /// A implementação padrão remove a sessão, mas não garante protecção
    /// contra gravações concorrentes. Armazenamentos concretos devem
    /// sobrescrever este método para garantir atomicidade.
    fn revoke(&self, session_id: &str) {
        self.remove(session_id);
    }

    /// Revokes a session and optionally defines how long the revocation lasts.
    ///
    /// When `ttl` is `None`, the revocation remains permanent.
    /// The default implementation delegates to `revoke`.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Identifier of the session to revoke.
    /// * `ttl` - Optional duration of the revocation.
    fn revoke_with_ttl(&self, session_id: &str, ttl: Option<Duration>) {
        let _ = ttl;
        self.revoke(session_id);
    }
}

/// Entrada armazenada, incluindo o momento de expiração.
struct SessionEntry {
    data: Value,
    expires_at: Option<Instant>,
}

/// Armazenamento de sessões em memória.
#[derive(Clone, Default)]
pub struct MemoryStore {
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    revoked: Arc<RwLock<HashMap<String, Option<Instant>>>>,
}

impl MemoryStore {
    /// Cria um armazenamento de sessões vazio.
    pub fn new() -> Self {
        Self::default()
    }
}

impl SessionStore for MemoryStore {
    fn get(&self, session_id: &str) -> Option<Value> {
        let mut sessions = self.sessions.write().ok()?;

        let expired = sessions
            .get(session_id)
            .and_then(|entry| entry.expires_at)
            .is_some_and(|expires_at| Instant::now() >= expires_at);

        if expired {
            sessions.remove(session_id);
            return None;
        }

        sessions.get(session_id).map(|entry| entry.data.clone())
    }

    fn set(&self, session_id: String, data: Value) {
        let Ok(mut sessions) = self.sessions.write() else {
            return;
        };

        let Ok(mut revoked) = self.revoked.write() else {
            return;
        };

        let now = Instant::now();

        revoked.retain(|_, expires_at| expires_at.map_or(true, |expires_at| now < expires_at));

        if revoked.contains_key(&session_id) {
            return;
        }

        let now = Instant::now();

        sessions.retain(|_, entry| entry.expires_at.map_or(true, |expires_at| now < expires_at));

        sessions.insert(
            session_id,
            SessionEntry {
                data,
                expires_at: None,
            },
        );
    }

    fn set_with_ttl(&self, session_id: String, data: Value, ttl: Duration) {
        let Ok(mut sessions) = self.sessions.write() else {
            return;
        };

        let Ok(mut revoked) = self.revoked.write() else {
            return;
        };

        let now = Instant::now();

        revoked.retain(|_, expires_at| expires_at.map_or(true, |expires_at| now < expires_at));

        if revoked.contains_key(&session_id) {
            return;
        }

        let now = Instant::now();

        sessions.retain(|_, entry| entry.expires_at.map_or(true, |expires_at| now < expires_at));

        let expires_at = now.checked_add(ttl);

        sessions.insert(session_id, SessionEntry { data, expires_at });
    }

    fn remove(&self, session_id: &str) {
        if let Ok(mut sessions) = self.sessions.write() {
            sessions.remove(session_id);
        }
    }

    fn revoke(&self, session_id: &str) {
        let Ok(mut sessions) = self.sessions.write() else {
            return;
        };

        let Ok(mut revoked) = self.revoked.write() else {
            return;
        };

        sessions.remove(session_id);

        revoked.insert(session_id.to_string(), None);
    }

    /// Revokes a session and optionally defines how long the revocation lasts.
    ///
    /// When `ttl` is `None`, the revocation remains permanent.
    /// The default implementation delegates to `revoke`.
    ///
    /// # Arguments
    ///
    /// * `session_id` - Identifier of the session to revoke.
    /// * `ttl` - Optional duration of the revocation.
    fn revoke_with_ttl(&self, session_id: &str, ttl: Option<Duration>) {
        let Ok(mut sessions) = self.sessions.write() else {
            return;
        };

        let Ok(mut revoked) = self.revoked.write() else {
            return;
        };

        sessions.remove(session_id);

        let expires_at = ttl.and_then(|duration| Instant::now().checked_add(duration));

        revoked.insert(session_id.to_string(), expires_at);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn expires_session_with_zero_ttl() {
        let store = MemoryStore::new();

        store.set_with_ttl(
            "session-1".to_string(),
            json!({"username": "fabricio"}),
            Duration::ZERO,
        );

        assert!(store.get("session-1").is_none());
    }

    #[test]
    fn stores_session_without_ttl() {
        let store = MemoryStore::new();

        store.set("session-2".to_string(), json!({"username": "fabricio"}));

        let data = store.get("session-2").unwrap();

        assert_eq!(data["username"], "fabricio");
    }

    #[test]
    fn removes_session_and_prevents_old_data_from_being_restored() {
        let store = MemoryStore::new();

        store.set("session-old".to_string(), json!({"username": "fabricio"}));

        store.remove("session-old");

        assert!(store.get("session-old").is_none());
    }

    #[test]
    fn revoked_session_rejects_writes_with_ttl() {
        let store = MemoryStore::new();

        store.set(
            "session-revoked".to_string(),
            json!({"username": "fabricio"}),
        );

        store.revoke("session-revoked");

        store.set_with_ttl(
            "session-revoked".to_string(),
            json!({"username": "restored"}),
            Duration::from_secs(3600),
        );

        assert!(store.get("session-revoked").is_none());
    }
}
