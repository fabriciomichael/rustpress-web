//! Gestão centralizada do ciclo de vida das sessões.

use std::{sync::Arc, time::Duration};

use uuid::Uuid;

use super::{Session, SessionStore};

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
    pub fn load(&self, session_id: &str) -> Session {
        match self.store.get(session_id) {
            Some(data) => Session::from_value(session_id.to_string(), data),
            None => self.create(),
        }
    }

    /// Guarda uma sessão no armazenamento.
    pub fn save(&self, session: &Session) {
        if session.is_destroyed() {
            self.destroy(session.id());
            return;
        }

        if !session.is_modified() {
            return;
        }

        let data = session.to_value();
        let session_id = session.id().to_string();

        match self.ttl {
            Some(ttl) => {
                self.store.set_with_ttl(session_id, data, ttl);
            }
            None => {
                self.store.set(session_id, data);
            }
        }
    }

    /// Remove uma sessão do armazenamento.
    pub fn destroy(&self, session_id: &str) {
        self.store.remove(session_id);
    }
    /// Regenera o identificador de uma sessão, preservando os seus dados.
    ///
    /// O identificador anterior é removido do armazenamento.
    pub fn regenerate(&self, session: &mut Session) {
        let old_id = session.id().to_string();

        self.store.revoke_with_ttl(&old_id, self.ttl);

        let mut new_session = Session::new(Uuid::new_v4().to_string());
        new_session.mark_modified();

        for (key, value) in session.to_value().as_object().into_iter().flatten() {
            let _ = new_session.insert(key.clone(), value.clone());
        }

        *session = new_session;
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
        manager.save(&session);

        let loaded = manager.load(&session_id);

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
        manager.save(&session);

        manager.destroy(&session_id);

        let loaded = manager.load(&session_id);

        assert_ne!(loaded.id(), session_id);
    }

    #[test]
    fn removes_destroyed_session_on_save() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let session_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        manager.save(&session);

        session.destroy();
        manager.save(&session);

        let loaded = manager.load(&session_id);

        assert_ne!(loaded.id(), session_id);
    }

    #[test]
    fn regenerates_session_id_and_preserves_data() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        let old_id = session.id().to_string();

        session.insert("username", "Fabricio").unwrap();
        session.insert("authenticated", true).unwrap();

        manager.save(&session);
        manager.regenerate(&mut session);

        assert_ne!(session.id(), old_id);

        assert_eq!(
            session.get::<String>("username"),
            Some("Fabricio".to_string())
        );

        assert_eq!(session.get::<bool>("authenticated"), Some(true));

        let old_session = manager.load(&old_id);
        assert_ne!(old_session.id(), old_id);

        // Guarda a sessão com o novo identificador.
        manager.save(&session);

        // Recupera a sessão utilizando o novo identificador.
        let new_id = session.id().to_string();
        let loaded_new_session = manager.load(&new_id);

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

        manager.save(&session);

        let restored_session = manager.load(&old_id);

        assert_ne!(restored_session.id(), old_id);
        assert!(restored_session.get::<String>("username").is_none());
    }

    #[test]
    fn does_not_restore_old_session_after_regeneration() {
        let manager = SessionManager::new(MemoryStore::new());

        let mut session = manager.create();
        session.insert("username", "Fabricio").unwrap();

        manager.save(&session);

        let old_id = session.id().to_string();

        // Simula duas requisições que carregaram a mesma sessão.
        let mut stale_session = manager.load(&old_id);
        let mut active_session = manager.load(&old_id);

        // A requisição activa regenera a sessão.
        manager.regenerate(&mut active_session);

        // A requisição antiga tenta guardar a sessão anterior.
        stale_session.insert("request", "old").unwrap();
        manager.save(&stale_session);

        // O identificador antigo não deve voltar a ser válido.
        let restored = manager.load(&old_id);

        assert_ne!(restored.id(), old_id);
    }
}
