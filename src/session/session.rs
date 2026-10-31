//! Gestão dos dados individuais de uma sessão.

use serde_json::{Map, Value};

/// Representa uma sessão HTTP.
#[derive(Debug, Clone)]
pub struct Session {
    /// Identificador único da sessão.
    id: String,

    /// Dados armazenados na sessão.
    data: Map<String, Value>,

    /// Indica se a sessão foi modificada.
    modified: bool,

    /// Indica se a sessão foi destruída.
    destroyed: bool,
}

impl Session {
    /// Cria uma nova sessão com um identificador e dados vazios.
    pub fn new(id: String) -> Self {
        Self {
            id,
            data: Map::new(),
            modified: false,
            destroyed: false,
        }
    }

    /// Devolve o identificador da sessão.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Obtém um valor da sessão e converte-o para o tipo pretendido.
    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.data
            .get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// Insere ou actualiza um valor na sessão.
    pub fn insert<T>(&mut self, key: impl Into<String>, value: T) -> Result<(), serde_json::Error>
    where
        T: serde::Serialize,
    {
        let value = serde_json::to_value(value)?;

        self.data.insert(key.into(), value);
        self.modified = true;

        Ok(())
    }

    /// Remove um valor da sessão.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let value = self.data.remove(key);

        if value.is_some() {
            self.modified = true;
        }

        value
    }

    /// Verifica se a sessão contém determinada chave.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Remove todos os dados da sessão.
    pub fn clear(&mut self) {
        if !self.data.is_empty() {
            self.data.clear();
            self.modified = true;
        }
    }

    /// Destrói a sessão.
    pub fn destroy(&mut self) {
        self.data.clear();
        self.destroyed = true;
        self.modified = true;
    }

    /// Verifica se a sessão foi modificada.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Verifica se a sessão foi destruída.
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Converte os dados da sessão para um valor JSON.
    pub(crate) fn to_value(&self) -> Value {
        Value::Object(self.data.clone())
    }

    /// Restaura os dados de uma sessão a partir de um valor JSON.
    pub(crate) fn from_value(id: String, value: Value) -> Self {
        let data = value.as_object().cloned().unwrap_or_default();

        Self {
            id,
            data,
            modified: false,
            destroyed: false,
        }
    }

    /// Marca a sessão como modificada.
    pub(crate) fn mark_modified(&mut self) {
        self.modified = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_new_session() {
        let session = Session::new("test-id".to_string());

        assert_eq!(session.id(), "test-id");
        assert!(!session.is_modified());
        assert!(!session.is_destroyed());
    }

    #[test]
    fn inserts_and_gets_values() {
        let mut session = Session::new("test-id".to_string());

        session.insert("username", "Fabricio").unwrap();
        session.insert("age", 25).unwrap();

        assert_eq!(
            session.get::<String>("username"),
            Some("Fabricio".to_string())
        );

        assert_eq!(session.get::<u32>("age"), Some(25));
        assert!(session.is_modified());
    }

    #[test]
    fn removes_values() {
        let mut session = Session::new("test-id".to_string());

        session.insert("username", "Fabricio").unwrap();
        let removed = session.remove("username");

        assert!(removed.is_some());
        assert!(!session.contains("username"));
    }

    #[test]
    fn clears_session() {
        let mut session = Session::new("test-id".to_string());

        session.insert("username", "Fabricio").unwrap();
        session.clear();

        assert!(!session.contains("username"));
        assert!(session.is_modified());
    }

    #[test]
    fn destroys_session() {
        let mut session = Session::new("test-id".to_string());

        session.insert("username", "Fabricio").unwrap();
        session.destroy();

        assert!(session.is_destroyed());
        assert!(session.is_modified());
        assert!(!session.contains("username"));
    }
}
