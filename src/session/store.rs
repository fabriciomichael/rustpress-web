//! Armazenamento interno dos dados das sessões.

use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant},
};

use serde_json::Value;

const REVOCATION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Erros que podem ocorrer durante operações de armazenamento de sessões.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStoreError {
    /// O lock interno do armazenamento foi envenenado.
    LockPoisoned,

    /// O tempo de vida não pode ser representado pelo relógio interno.
    TtlOverflow,

    /// A tentativa de escrita utilizou um ID de sessão revogado.
    RevokedSession,
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::LockPoisoned => {
                write!(f, "O armazenamento de sessões está indisponível.")
            }
            Self::TtlOverflow => {
                write!(f, "O tempo de vida da sessão é demasiado grande.")
            }
            Self::RevokedSession => write!(f, "Não é possível gravar dados numa sessão revogada."),
        }
    }
}

impl std::error::Error for SessionStoreError {}

/// Interface para armazenamento dos dados das sessões.
pub trait SessionStore: Send + Sync {
    /// Obtém os dados de uma sessão pelo seu identificador.
    fn get(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError>;

    /// Guarda ou actualiza os dados de uma sessão.
    fn set(&self, session_id: String, data: Value) -> Result<(), SessionStoreError>;

    /// Guarda uma sessão com um tempo de vida limitado.
    fn set_with_ttl(
        &self,
        session_id: String,
        data: Value,
        ttl: Duration,
    ) -> Result<(), SessionStoreError>;

    /// Remove uma sessão do armazenamento.
    fn remove(&self, session_id: &str) -> Result<(), SessionStoreError>;

    /// Revoga uma sessão, impedindo gravações posteriores com o mesmo ID.
    fn revoke(&self, session_id: &str) -> Result<(), SessionStoreError>;

    /// Revoga uma sessão e define opcionalmente a duração da revogação.
    fn revoke_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError>;
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
    last_cleanup: Arc<Mutex<Option<Instant>>>,
}

impl MemoryStore {
    /// Cria um armazenamento de sessões vazio.
    pub fn new() -> Self {
        Self::default()
    }

    fn revocation_expiry(ttl: Option<Duration>) -> Result<Option<Instant>, SessionStoreError> {
        let Some(duration) = ttl else {
            return Ok(None);
        };

        let duration = duration.max(REVOCATION_TTL);

        Instant::now()
            .checked_add(duration)
            .map(Some)
            .ok_or(SessionStoreError::TtlOverflow)
    }

    /// Remove sessões e registos de revogação expirados.
    ///
    /// Retorna a quantidade total de entradas removidas.
    pub fn cleanup(&self) -> Result<usize, SessionStoreError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut revoked = self
            .revoked
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let now = Instant::now();

        // Revoga e remove as sessões expiradas.
        let removed_sessions = Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Remove os registos de revogação expirados.
        let revoked_before = revoked.len();

        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        let removed_revocations = revoked_before - revoked.len();

        Ok(removed_sessions + removed_revocations)
    }
    /// Executa a limpeza quando o intervalo configurado tiver decorrido.
    fn cleanup_if_needed(&self) -> Result<(), SessionStoreError> {
        let mut last_cleanup = self
            .last_cleanup
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let now = Instant::now();

        let should_cleanup =
            last_cleanup.is_none_or(|last| now.duration_since(last) >= CLEANUP_INTERVAL);

        if should_cleanup {
            self.cleanup()?;

            *last_cleanup = Some(Instant::now());
        }

        Ok(())
    }

    fn revoke_expired_sessions(
        sessions: &mut HashMap<String, SessionEntry>,
        revoked: &mut HashMap<String, Option<Instant>>,
        now: Instant,
    ) -> Result<usize, SessionStoreError> {
        // Identifica as sessões expiradas.
        let expired_ids: Vec<String> = sessions
            .iter()
            .filter(|(_, entry)| entry.expires_at.is_some_and(|expiry| now >= expiry))
            .map(|(session_id, _)| session_id.clone())
            .collect();

        if expired_ids.is_empty() {
            return Ok(0);
        }

        // Regista permanentemente os IDs expirados como revogados.
        for session_id in &expired_ids {
            revoked
                .entry(session_id.clone())
                .and_modify(|expiry| *expiry = None)
                .or_insert(None);
        }

        // Remove as sessões expiradas.
        sessions.retain(|_, entry| entry.expires_at.is_none_or(|expiry| now < expiry));

        Ok(expired_ids.len())
    }
}

impl SessionStore for MemoryStore {
    fn get(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let expired = sessions
            .get(session_id)
            .and_then(|entry| entry.expires_at)
            .is_some_and(|expires_at| Instant::now() >= expires_at);

        if expired {
            // Calcula o prazo antes de modificar o armazenamento.

            // Mantém a ordem global de aquisição dos locks.
            let mut revoked = self
                .revoked
                .write()
                .map_err(|_| SessionStoreError::LockPoisoned)?;

            // Só modifica os mapas depois de todas as operações
            // que podem devolver erros terem sido concluídas.
            sessions.remove(session_id);

            revoked
                .entry(session_id.to_string())
                .and_modify(|expiry| *expiry = None)
                .or_insert(None);

            return Ok(None);
        }

        Ok(sessions.get(session_id).map(|entry| entry.data.clone()))
    }

    fn set(&self, session_id: String, data: Value) -> Result<(), SessionStoreError> {
        self.cleanup_if_needed()?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut revoked = self
            .revoked
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let now = Instant::now();

        // Remove revogações que já expiraram.
        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        // Revoga e remove as sessões expiradas.
        Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Impede a reutilização de IDs revogados.
        if revoked.contains_key(&session_id) {
            return Err(SessionStoreError::RevokedSession);
        }

        // Guarda a nova sessão.
        sessions.insert(
            session_id,
            SessionEntry {
                data,
                expires_at: None,
            },
        );

        Ok(())
    }

    fn set_with_ttl(
        &self,
        session_id: String,
        data: Value,
        ttl: Duration,
    ) -> Result<(), SessionStoreError> {
        self.cleanup_if_needed()?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut revoked = self
            .revoked
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let now = Instant::now();

        let expires_at = now.checked_add(ttl).ok_or(SessionStoreError::TtlOverflow)?;

        // Remove as revogações que já expiraram.
        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        // Revoga e remove as sessões expiradas.
        Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Não permite reutilizar um ID revogado.
        if revoked.contains_key(&session_id) {
            return Err(SessionStoreError::RevokedSession);
        }

        // Guarda a nova sessão.
        sessions.insert(
            session_id,
            SessionEntry {
                data,
                expires_at: Some(expires_at),
            },
        );

        Ok(())
    }
    fn remove(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        sessions.remove(session_id);

        Ok(())
    }

    fn revoke(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut revoked = self
            .revoked
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        sessions.remove(session_id);

        revoked
            .entry(session_id.to_string())
            .and_modify(|expiry| *expiry = None)
            .or_insert(None);

        Ok(())
    }

    fn revoke_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError> {
        let expires_at = Self::revocation_expiry(ttl)?;

        let mut sessions = self
            .sessions
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut revoked = self
            .revoked
            .write()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        sessions.remove(session_id);

        match expires_at {
            None => {
                revoked.insert(session_id.to_string(), None);
            }
            Some(expiry) => {
                revoked
                    .entry(session_id.to_string())
                    .and_modify(|existing_expiry| {
                        if let Some(current_expiry) = existing_expiry
                            && *current_expiry < expiry
                        {
                            *existing_expiry = Some(expiry);
                        }
                    })
                    .or_insert(Some(expiry));
            }
        }

        Ok(())
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

        store
            .set_with_ttl(
                "session-1".to_string(),
                json!({"username": "fabricio"}),
                Duration::ZERO,
            )
            .unwrap();

        assert!(store.get("session-1").unwrap().is_none());
    }

    #[test]
    fn stores_session_without_ttl() {
        let store = MemoryStore::new();

        store
            .set("session-2".to_string(), json!({"username": "fabricio"}))
            .unwrap();

        let data = store.get("session-2").unwrap().unwrap();

        assert_eq!(data["username"], "fabricio");
    }

    #[test]
    fn removes_session_and_prevents_old_data_from_being_restored() {
        let store = MemoryStore::new();

        store
            .set("session-old".to_string(), json!({"username": "fabricio"}))
            .unwrap();

        store.remove("session-old").unwrap();

        assert!(store.get("session-old").unwrap().is_none());
    }

    #[test]
    fn revoked_session_rejects_writes_with_ttl() {
        let store = MemoryStore::new();

        store
            .set(
                "session-revoked".to_string(),
                json!({"username": "fabricio"}),
            )
            .unwrap();

        store.revoke("session-revoked").unwrap();

        let result = store.set_with_ttl(
            "session-revoked".to_string(),
            json!({"user_id": 123}),
            Duration::from_secs(60),
        );

        assert_eq!(result, Err(SessionStoreError::RevokedSession));
    }

    #[test]
    fn removes_session_after_ttl_expires() {
        let store = MemoryStore::new();
        let session_id = "session-expiration-test".to_string();

        store
            .set_with_ttl(
                session_id.clone(),
                json!({"user": "Fabrício"}),
                Duration::from_millis(50),
            )
            .unwrap();

        assert!(store.get(&session_id).unwrap().is_some());

        std::thread::sleep(Duration::from_millis(100));

        assert!(store.get(&session_id).unwrap().is_none());
    }

    #[test]
    fn expired_session_id_cannot_recover_previous_data() {
        let store = MemoryStore::new();
        let session_id = "expired-session-id".to_string();

        store
            .set_with_ttl(
                session_id.clone(),
                json!({
                    "user_id": 123,
                    "role": "admin"
                }),
                Duration::from_millis(50),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        // O ID antigo não deve recuperar os dados anteriores.
        assert!(store.get(&session_id).unwrap().is_none());

        // Mesmo que tentem guardar dados usando o mesmo ID,
        // a store não deve recuperar a sessão antiga.
        // Uma tentativa de reutilizar o ID revogado deve falhar.
        let result = store.set(session_id.clone(), json!({"user_id": 999}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // Os dados antigos continuam inacessíveis.
        assert!(store.get(&session_id).unwrap().is_none());
    }

    #[test]
    fn cleanup_removes_expired_sessions() {
        let store = MemoryStore::new();

        store
            .set_with_ttl(
                "expired-session".to_string(),
                json!({"user": "fabricio"}),
                Duration::from_millis(50),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        let removed = store.cleanup().unwrap();

        assert_eq!(removed, 1);
        assert!(store.get("expired-session").unwrap().is_none());
    }

    #[test]
    fn cleanup_preserves_valid_sessions() {
        let store = MemoryStore::new();

        store
            .set_with_ttl(
                "valid-session".to_string(),
                json!({"user": "fabricio"}),
                Duration::from_secs(3600),
            )
            .unwrap();

        let removed = store.cleanup().unwrap();

        assert_eq!(removed, 0);
        assert!(store.get("valid-session").unwrap().is_some());
    }

    #[test]
    fn repeated_temporary_revocation_preserves_longer_ttl() {
        let store = MemoryStore::default();

        store
            .revoke_with_ttl("session-123", Some(Duration::from_secs(48 * 60 * 60)))
            .unwrap();

        let original_expiry = *store.revoked.read().unwrap().get("session-123").unwrap();

        store
            .revoke_with_ttl("session-123", Some(Duration::from_secs(24 * 60 * 60)))
            .unwrap();

        let updated_expiry = *store.revoked.read().unwrap().get("session-123").unwrap();

        assert_eq!(original_expiry, updated_expiry);
    }

    #[test]
    fn permanent_revocation_supersedes_temporary_revocation() {
        let store = MemoryStore::default();

        store
            .revoke_with_ttl("session-123", Some(Duration::from_secs(48 * 60 * 60)))
            .unwrap();

        store.revoke("session-123").unwrap();

        let revoked = store.revoked.read().unwrap();

        assert_eq!(revoked.get("session-123"), Some(&None));
    }

    #[test]
    fn automatic_cleanup_skips_when_interval_has_not_elapsed() {
        let store = MemoryStore::new();

        {
            let mut revoked = store.revoked.write().unwrap();
            revoked.insert(
                "expired-id".to_string(),
                Some(Instant::now() - Duration::from_secs(1)),
            );
        }

        {
            let mut last_cleanup = store.last_cleanup.lock().unwrap();
            *last_cleanup = Some(Instant::now());
        }

        store.cleanup_if_needed().unwrap();

        assert!(
            store.revoked.read().unwrap().contains_key("expired-id"),
            "A limpeza não deveria ocorrer antes de terminar o intervalo."
        );
    }

    #[test]
    fn automatic_cleanup_runs_after_interval() {
        let store = MemoryStore::new();

        {
            let mut revoked = store.revoked.write().unwrap();
            revoked.insert(
                "expired-id".to_string(),
                Some(Instant::now() - Duration::from_secs(1)),
            );
        }

        {
            let mut last_cleanup = store.last_cleanup.lock().unwrap();
            *last_cleanup = Some(Instant::now() - CLEANUP_INTERVAL - Duration::from_secs(1));
        }

        store.cleanup_if_needed().unwrap();

        assert!(
            !store.revoked.read().unwrap().contains_key("expired-id"),
            "A limpeza deveria remover a revogação expirada."
        );
    }

    #[test]
    fn set_triggers_automatic_cleanup() {
        let store = MemoryStore::new();

        {
            let mut revoked = store.revoked.write().unwrap();
            revoked.insert(
                "expired-id".to_string(),
                Some(Instant::now() - Duration::from_secs(1)),
            );
        }

        {
            let mut last_cleanup = store.last_cleanup.lock().unwrap();
            *last_cleanup = Some(Instant::now() - CLEANUP_INTERVAL - Duration::from_secs(1));
        }

        store
            .set(
                "new-session".to_string(),
                serde_json::json!({"user": "Fabrício"}),
            )
            .unwrap();

        assert!(
            !store.revoked.read().unwrap().contains_key("expired-id"),
            "set() deveria desencadear a limpeza automática."
        );
    }

    #[test]
    fn set_with_ttl_triggers_automatic_cleanup() {
        let store = MemoryStore::new();

        {
            let mut revoked = store.revoked.write().unwrap();
            revoked.insert(
                "expired-id".to_string(),
                Some(Instant::now() - Duration::from_secs(1)),
            );
        }

        {
            let mut last_cleanup = store.last_cleanup.lock().unwrap();
            *last_cleanup = Some(Instant::now() - CLEANUP_INTERVAL - Duration::from_secs(1));
        }

        store
            .set_with_ttl(
                "new-session".to_string(),
                serde_json::json!({"user": "Fabrício"}),
                Duration::from_secs(300),
            )
            .unwrap();

        assert!(
            !store.revoked.read().unwrap().contains_key("expired-id"),
            "set_with_ttl() deveria desencadear a limpeza automática."
        );
    }

    #[test]
    fn cleanup_revokes_expired_session_id() {
        let store = MemoryStore::default();

        store
            .set_with_ttl(
                "expired-session".to_string(),
                serde_json::json!({"user": 1}),
                Duration::from_millis(1),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(10));

        store.cleanup().unwrap();

        // Confirma a revogação permanente.
        assert_eq!(
            store.revoked.read().unwrap().get("expired-session"),
            Some(&None)
        );

        // O identificador não pode ser reutilizado.
        let result = store.set(
            "expired-session".to_string(),
            serde_json::json!({"user": 2}),
        );

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        assert!(store.get("expired-session").unwrap().is_none());
    }

    #[test]
    fn set_with_ttl_revokes_other_expired_session_ids() {
        let store = MemoryStore::default();

        store
            .set_with_ttl(
                "expired-session".to_string(),
                serde_json::json!({"user": 1}),
                Duration::from_millis(1),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(10));

        store
            .set_with_ttl(
                "new-session".to_string(),
                serde_json::json!({"user": 2}),
                Duration::from_secs(60),
            )
            .unwrap();

        let result = store.set(
            "expired-session".to_string(),
            serde_json::json!({"user": 3}),
        );

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        assert!(store.get("expired-session").unwrap().is_none());
        assert!(store.get("new-session").unwrap().is_some());
    }

    #[test]
    fn set_revokes_expired_session_id() {
        let store = MemoryStore::default();

        store
            .set_with_ttl(
                "expired-session".to_string(),
                serde_json::json!({"user": 1}),
                Duration::from_millis(1),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(10));

        let result = store.set(
            "expired-session".to_string(),
            serde_json::json!({"user": 2}),
        );

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // Confirma que a expiração provocou uma revogação permanente.
        assert_eq!(
            store.revoked.read().unwrap().get("expired-session"),
            Some(&None)
        );

        assert!(store.get("expired-session").unwrap().is_none());
    }
}
