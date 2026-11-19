//! Management of individual session data.

use serde_json::{Map, Value};

/// Represents an HTTP session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier.
    id: String,

    /// Data stored in the session.
    data: Map<String, Value>,

    /// Indicates whether the session was modified.
    modified: bool,

    /// Indicates whether the session was destroyed.
    destroyed: bool,
}

impl Session {
    /// Creates a new session with an identifier and empty data.
    pub fn new(id: String) -> Self {
        Self {
            id,
            data: Map::new(),
            modified: false,
            destroyed: false,
        }
    }

    /// Returns the session identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Gets a session value and converts it to the requested type.
    pub fn get<T>(&self, key: &str) -> Option<T>
    where
        T: serde::de::DeserializeOwned,
    {
        self.data
            .get(key)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    /// Inserts or updates a value in the session.
    pub fn insert<T>(&mut self, key: impl Into<String>, value: T) -> Result<(), serde_json::Error>
    where
        T: serde::Serialize,
    {
        let value = serde_json::to_value(value)?;

        self.data.insert(key.into(), value);
        self.modified = true;

        Ok(())
    }

    /// Removes a value from the session.
    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let value = self.data.remove(key);

        if value.is_some() {
            self.modified = true;
        }

        value
    }

    /// Checks whether the session contains the specified key.
    pub fn contains(&self, key: &str) -> bool {
        self.data.contains_key(key)
    }

    /// Removes all data from the session.
    pub fn clear(&mut self) {
        if !self.data.is_empty() {
            self.data.clear();
            self.modified = true;
        }
    }

    /// Destroys the session.
    pub fn destroy(&mut self) {
        self.data.clear();
        self.destroyed = true;
        self.modified = true;
    }

    /// Checks whether the session was modified.
    pub fn is_modified(&self) -> bool {
        self.modified
    }

    /// Checks whether the session was destroyed.
    pub fn is_destroyed(&self) -> bool {
        self.destroyed
    }

    /// Converts the session data to a JSON value.
    pub(crate) fn to_value(&self) -> Value {
        Value::Object(self.data.clone())
    }

    /// Restores session data from a JSON value.
    pub(crate) fn from_value(id: String, value: Value) -> Self {
        let data = value.as_object().cloned().unwrap_or_default();

        Self {
            id,
            data,
            modified: false,
            destroyed: false,
        }
    }

    /// Marks the session as modified.
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
