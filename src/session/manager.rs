//! Centralized session lifecycle management.

use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use super::{Session, SessionStore, SessionStoreError};

/// Handles session creation, retrieval, and persistence.
#[derive(Clone)]
pub struct SessionManager {
    store: Arc<dyn SessionStore>,
    ttl: Option<Duration>,
}

impl SessionManager {
    /// Creates a session manager with the provided storage.
    pub fn new<S>(store: S) -> Self
    where
        S: SessionStore + 'static,
    {
        Self {
            store: Arc::new(store),
            ttl: None,
        }
    }

    /// Creates a session manager with a configured time to live.
    pub fn with_ttl<S>(store: S, ttl: Duration) -> Self
    where
        S: SessionStore + 'static,
    {
        Self {
            store: Arc::new(store),
            ttl: Some(ttl),
        }
    }

    /// Creates a new session with a random identifier.
    pub fn create(&self) -> Session {
        Session::new(Uuid::new_v4().to_string())
    }

    /// Retrieves an existing session or creates a new one.
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

    /// Saves a session to storage.
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

    /// Removes a session from storage.
    pub fn destroy(&self, session_id: &str) -> Result<(), SessionStoreError> {
        self.store.revoke_with_ttl(session_id, self.ttl)
    }

    /// Regenerates a session identifier while preserving its data.
    ///
    /// The previous identifier is removed from storage.
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

        // Save the session with the new identifier.
        manager.save(&session).unwrap();

        // Retrieve the session using the new identifier.
        let new_id = session.id().to_string();
        let loaded_new_session = manager.load(&new_id).unwrap();

        // Confirm that the data was persisted.
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

        // Simulate two requests that loaded the same session.
        let mut stale_session = manager.load(&old_id).unwrap();
        let mut active_session = manager.load(&old_id).unwrap();

        // The active request regenerates the session.
        manager.regenerate(&mut active_session).unwrap();

        // Try to reuse the old identifier.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 999}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // The old request tries to save the previous session.
        stale_session.insert("request", "old").unwrap();

        let result = manager.save(&stale_session);

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // The old identifier must not become valid again.
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

        // Try to save data again using the revoked ID.
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

        // Try to restore data using the destroyed ID.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 456}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // The manager should create a session with another identifier.
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

        // The reuse attempt should be blocked.
        let result = store.set(old_id.clone(), serde_json::json!({"user_id": 456}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        let new_session = manager.load(&old_id).unwrap();

        assert_ne!(new_session.id(), old_id);
        assert!(!new_session.contains("user_id"));
    }
}
