//! Gestão centralizada do ciclo de vida das sessões.

use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use super::{Session, SessionStore, SessionStoreError};

/// Gere a criação, recuperação e persistência das sessões.
#[derive(Clone)]
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    ttl: Option<Duration>,
}

impl SessionManager {
    /// Cria um gestor de sessões com o armazenamento fornecido.
    pub fn new<S>(store: S) -> Self
    where
        S: SessionStore + 'static,
    {
        Self {
            store: Arc::new(store),
            ttl: None,
        }
    }

    /// Cria um gestor de sessões com um tempo de vida definido.
    pub fn with_ttl<S>(store: S, ttl: Duration) -> Self
    where
        S: SessionStore + 'static,
    {
        Self {
            store: Arc::new(store),
            ttl: Some(ttl),
        }
    }

    /// Cria uma nova sessão com um identificador aleatório.
    pub fn create(&self) -> Session {
        Session::new(Uuid::new_v4().to_string())
    }

    /// Recupera uma sessão existente ou cria uma nova.
    pub fn load(&self, session_id: &str) -> Result<Session, SessionStoreError> {
        let Ok(session_uuid) = Uuid::parse_str(session_id) else {
            return Ok(self.create());
        };

        if session_uuid.get_version() != Some(uuid::Version::Random) {
            return Ok(self.create());
        }

        match self.store.get(session_id)? {
            Some(data) => Ok(Session::from_value(session_id.to_string(), data)),
            None => Ok(self.create()),
        }
    }

    /// Guarda uma sessão no armazenamento.
    pub fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
        if session.is_destroyed() {
            return self.destroy(session.id());
        }

        if !session.is_modified() {
            return Ok(());
        }

        let data = session.to_value();
        let session_id = session.id().to_string();

        match self.ttl {
            Some(ttl) => self.store.set_with_ttl(session_id, data, ttl),
            None => self.store.set(session_id, data),
        }
    }

    /// Remove uma sessão do armazenamento.
    pub fn destroy(&self, session_id: &str) -> Result<(), SessionStoreError> {
        self.store.revoke_with_ttl(session_id, self.ttl)
    }

    /// Regenera o identificador de uma sessão, preservando os seus dados.
    ///
    /// O identificador anterior é removido do armazenamento.
    pub fn regenerate(&self, session: &mut Session) -> Result<(), SessionStoreError> {
        let old_id = session.id().to_string();

        let mut new_session = Session::new(Uuid::new_v4().to_string());

        for (key, value) in session.to_value().as_object().into_iter().flatten() {
            new_session
                .insert(key.clone(), value.clone())
                .expect("Os dados serializados da sessão devem ser válidos.");
        }

        new_session.mark_modified();

        self.store.revoke_with_ttl(&old_id, self.ttl)?;

        *session = new_session;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::MemoryStore;
    use std::time::Duration;

    #[test]
    fn creates_session_with_unique_id() {
        let manager = SessionManager::new(MemoryStore::new());

        let session1 = manager.create();
        let session2 = manager.create();

        assert!(!session1.id().is_empty());
        assert_ne!(session1.id(), session2.id());
    }

    #[test]
    fn saves_and_loads_session() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let session_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        manager.save(&session).unwrap();

        let loaded = manager.load(&session_id).unwrap();

        assert_eq!(
            loaded.get::<String>("username"),
            Some("Fabricio".to_string())
        );
    }

    #[test]
    fn destroys_session() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let session_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        manager.save(&session).unwrap();

        manager.destroy(&session_id).unwrap();

        let loaded = manager.load(&session_id).unwrap();

        assert_ne!(loaded.id(), session_id);
    }

    #[test]
    fn removes_destroyed_session_on_save() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let session_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        manager.save(&session).unwrap();

        session.destroy();
        manager.save(&session).unwrap();

        let loaded = manager.load(&session_id).unwrap();

        assert_ne!(loaded.id(), session_id);
    }

    #[test]
    fn regenerates_session_id_and_preserves_data() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let old_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        session.insert("authenticated", true).unwrap();

        manager.save(&session).unwrap();
        manager.regenerate(&mut session).unwrap();

        assert_ne!(session.id(), old_id);

        assert_eq!(
            session.get::<String>("username"),
            Some("Fabricio".to_string())
        );

        assert_eq!(session.get::<bool>("authenticated"), Some(true));

        let old_session = manager.load(&old_id).unwrap();
        assert_ne!(old_session.id(), old_id);

        // Guarda a sessão com o novo identificador.
        manager.save(&session).unwrap();

        // Recupera a sessão utilizando o novo identificador.
        let new_id = session.id().to_string();
        let loaded_new_session = manager.load(&new_id).unwrap();

        // Confirma que os dados foram persistidos.
        assert_eq!(
            loaded_new_session.get::<String>("username"),
            Some("Fabricio".to_string())
        );

        assert_eq!(loaded_new_session.get::<bool>("authenticated"), Some(true));
    }

    #[test]
    fn creates_new_session_when_previous_session_has_expired() {
        let manager = SessionManager::with_ttl(MemoryStore::new(), Duration::ZERO);

        let mut session = manager.create();

        let old_id = session.id().to_string();

        session.insert("username", "fabricio").unwrap();

        manager.save(&session).unwrap();

        let restored_session = manager.load(&old_id).unwrap();

        assert_ne!(restored_session.id(), old_id);
        assert!(restored_session.get::<String>("username").is_none());
    }

    #[test]
    fn does_not_restore_old_session_after_regeneration() {
        let store = MemoryStore::new();
        let manager = SessionManager::new(store.clone());

        let mut session = manager.create();
        session.insert("username", "Fabricio").unwrap();

        manager.save(&session).unwrap();

        let old_id = session.id().to_string();

        // Simula duas requisições que carregaram a mesma sessão.
        let mut stale_session = manager.load(&old_id).unwrap();
        let mut active_session = manager.load(&old_id).unwrap();

        // A requisição activa regenera a sessão.
        manager.regenerate(&mut active_session).unwrap();

        // Tenta reutilizar o identificador antigo.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 999}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // A requisição antiga tenta guardar a sessão anterior.
        stale_session.insert("request", "old").unwrap();

        let result = manager.save(&stale_session);

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // O identificador antigo não deve voltar a ser válido.
        let restored = manager.load(&old_id).unwrap();

        assert_ne!(restored.id(), old_id);
    }

    #[test]
    fn rejects_invalid_session_ids() {
        let manager = SessionManager::new(MemoryStore::new());

        let invalid_ids = [
            "",
            "abc",
            "123",
            "not-a-uuid",
            "550e8400-e29b-41d4-a716-446655440000",
        ];

        for invalid_id in invalid_ids {
            let session = manager.load(invalid_id).unwrap();

            assert_ne!(
                session.id(),
                invalid_id,
                "O identificador inválido foi aceite: {invalid_id}"
            );
        }
    }

    #[test]
    fn accepts_valid_v4_session_id() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let session_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        manager.save(&session).unwrap();

        let loaded = manager.load(&session_id).unwrap();

        assert_eq!(loaded.id(), session_id);
        assert_eq!(
            loaded.get::<String>("username"),
            Some("Fabricio".to_string())
        );
    }

    #[test]
    fn creates_new_session_when_session_id_has_expired() {
        let store = MemoryStore::new();
        let manager = SessionManager::with_ttl(store.clone(), Duration::from_millis(50));

        let old_id = Uuid::new_v4().to_string();

        store
            .set_with_ttl(
                old_id.clone(),
                serde_json::json!({"user_id": 123}),
                Duration::from_millis(50),
            )
            .unwrap();

        std::thread::sleep(Duration::from_millis(100));

        let session = manager.load(&old_id).unwrap();

        assert_ne!(session.id(), old_id);
        assert!(!session.contains("user_id"));
    }

    #[test]
    fn revoked_session_id_cannot_be_reused() {
        let store = MemoryStore::new();
        let session_id = "revoked-session-id".to_string();

        store
            .set(session_id.clone(), serde_json::json!({"user_id": 123}))
            .unwrap();

        store.revoke(&session_id).unwrap();

        assert!(store.get(&session_id).unwrap().is_none());

        // Tenta guardar novamente dados com o ID revogado.
        let result = store.set(session_id.clone(), serde_json::json!({"user_id": 456}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        assert!(store.get(&session_id).unwrap().is_none());
    }

    #[test]
    fn destroyed_session_id_cannot_be_reused() {
        let store = MemoryStore::new();
        let manager = SessionManager::new(store.clone());

        let mut session = manager.create();
        let old_id = session.id().to_string();

        session.insert("user_id", serde_json::json!(123)).unwrap();
        manager.save(&session).unwrap();

        manager.destroy(&old_id).unwrap();

        // Tenta restaurar dados usando o ID destruído.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 456}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // O gestor deve criar uma sessão com outro identificador.
        let new_session = manager.load(&old_id).unwrap();

        assert_ne!(new_session.id(), old_id);
        assert!(!new_session.contains("user_id"));
    }

    #[test]
    fn destroyed_session_id_remains_revoked_during_ttl() {
        let store = MemoryStore::new();
        let manager = SessionManager::with_ttl(store.clone(), Duration::from_secs(60));

        let mut session = manager.create();
        let old_id = session.id().to_string();

        session.insert("user_id", serde_json::json!(123)).unwrap();
        manager.save(&session).unwrap();

        manager.destroy(&old_id).unwrap();

        // A tentativa de reutilização deve ser bloqueada.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 456}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        let new_session = manager.load(&old_id).unwrap();

        assert_ne!(new_session.id(), old_id);
        assert!(!new_session.contains("user_id"));
    }
}
