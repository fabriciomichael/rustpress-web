//! Internal storage for session data.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, RwLock},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use serde_json::Value;

const REVOCATION_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);

/// Errors that can occur during session storage operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStoreError {
    /// The storage's internal lock was poisoned.
    LockPoisoned,

    /// The time to live cannot be represented by the internal clock.
    TtlOverflow,

    /// The write attempt used a revoked session ID.
    RevokedSession,

    /// An error occurred during a file operation.
    Io(String),

    /// An error occurred while serializing or deserializing data.
    Serialization(String),
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
            Self::Io(message) => write!(f, "Erro de acesso ao armazenamento: {message}"),
            Self::Serialization(message) => {
                write!(f, "Erro ao processar os dados das sessões: {message}")
            }
        }
    }
}

impl std::error::Error for SessionStoreError {}

/// Interface for storing session data.
pub trait SessionStore: Send + Sync {
    /// Gets session data by its identifier.
    fn get(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError>;

    /// Saves or updates session data.
    fn set(&self, session_id: String, data: Value) -> Result<(), SessionStoreError>;

    /// Saves a session with a limited time to live.
    fn set_with_ttl(
        &self,
        session_id: String,
        data: Value,
        ttl: Duration,
    ) -> Result<(), SessionStoreError>;

    /// Removes a session from storage.
    fn remove(&self, session_id: &str) -> Result<(), SessionStoreError>;

    /// Revokes a session, preventing further writes with the same ID.
    fn revoke(&self, session_id: &str) -> Result<(), SessionStoreError>;

    /// Revokes a session and optionally defines the revocation duration.
    fn revoke_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError>;
}
/// Stored entry, including its expiration time.
struct SessionEntry {
    data: Value,
    expires_at: Option<Instant>,
}

/// Session entry prepared for file persistence.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FileSessionEntry {
    data: Value,
    expires_at: Option<u64>,
}

/// Complete state persisted by FileStore.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct FileStoreData {
    sessions: HashMap<String, FileSessionEntry>,
    revoked: HashMap<String, Option<u64>>,
}

/// Persistent session storage in a JSON file.
pub struct FileStore {
    path: PathBuf,
    lock_path: PathBuf,
    state: Mutex<FileStoreData>,
}

impl FileStore {
    /// Opens persistent session storage.
    ///
    /// If the main file does not exist, tries to recover a backup
    /// left by a previous interruption.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, SessionStoreError> {
        let cutoff = SystemTime::now()
            .checked_sub(Duration::from_secs(24 * 60 * 60))
            .unwrap_or(UNIX_EPOCH);

        Self::new_with_cutoff(path, cutoff)
    }

    fn new_with_cutoff(
        path: impl AsRef<Path>,
        cutoff: SystemTime,
    ) -> Result<Self, SessionStoreError> {
        let path = path.as_ref().to_path_buf();
        let lock_path = path.with_extension("lock");
        let _file_lock = Self::acquire_file_lock(&lock_path)?;

        Self::cleanup_temporary_files(&path, cutoff)?;

        let state = Self::load_state(&path)?;

        Ok(Self {
            path,
            lock_path,
            state: Mutex::new(state),
        })
    }

    /// Acquires an exclusive lock on the auxiliary file.
    fn acquire_file_lock(lock_path: &Path) -> Result<fs::File, SessionStoreError> {
        use fs2::FileExt;

        if let Some(parent) = lock_path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| SessionStoreError::Io(error.to_string()))?;
        }

        let lock_file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(lock_path)
            .map_err(|error| SessionStoreError::Io(error.to_string()))?;

        lock_file
            .lock_exclusive()
            .map_err(|error| SessionStoreError::Io(error.to_string()))?;

        Ok(lock_file)
    }

    /// Loads the persisted state and recovers a backup when necessary.
    fn load_state(path: &Path) -> Result<FileStoreData, SessionStoreError> {
        let state = match fs::read(path) {
            Ok(contents) => match serde_json::from_slice(&contents) {
                Ok(state) => state,

                Err(error) => {
                    let original_error = SessionStoreError::Serialization(error.to_string());

                    // Preserve the corrupted main file.
                    let corrupt_path =
                        path.with_extension(format!("corrupt-{}", uuid::Uuid::new_v4()));

                    fs::rename(path, &corrupt_path)
                        .map_err(|error| SessionStoreError::Io(error.to_string()))?;

                    match Self::recover_backup(path) {
                        Ok(Some(state)) => state,

                        Ok(None) => {
                            fs::rename(&corrupt_path, path).map_err(|restore_error| {
                                SessionStoreError::Io(format!(
                                    "Não foi possível restaurar o ficheiro corrompido: {restore_error}"
                                ))
                            })?;

                            return Err(original_error);
                        }

                        Err(recovery_error) => {
                            fs::rename(&corrupt_path, path).map_err(|restore_error| {
                                SessionStoreError::Io(format!(
                                    "Falha na recuperação ({recovery_error}). \
                                     Também não foi possível restaurar o ficheiro original: \
                                     {restore_error}"
                                ))
                            })?;

                            return Err(recovery_error);
                        }
                    }
                }
            },

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::recover_backup(path)?.unwrap_or_default()
            }

            Err(error) => {
                return Err(SessionStoreError::Io(error.to_string()));
            }
        };

        Ok(state)
    }

    /// Searches for and recovers a backup.
    fn recover_backup(path: &Path) -> Result<Option<FileStoreData>, SessionStoreError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));

        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                SessionStoreError::Io("Nome do ficheiro de sessões inválido.".to_string())
            })?;

        let prefix = format!("{stem}.backup-");

        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(None);
            }
            Err(error) => {
                return Err(SessionStoreError::Io(error.to_string()));
            }
        };

        let mut backups = Vec::new();

        for entry in entries {
            let entry = entry.map_err(|error| SessionStoreError::Io(error.to_string()))?;

            if entry.file_name().to_string_lossy().starts_with(&prefix)
                && entry.file_type().is_ok_and(|file_type| file_type.is_file())
            {
                backups.push(entry.path());
            }
        }

        // Prioritize the most recent backups.
        backups.sort_by_key(|backup| {
            fs::metadata(backup)
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH)
        });

        backups.reverse();

        let mut last_error = None;

        for backup_path in backups {
            let contents = match fs::read(&backup_path) {
                Ok(contents) => contents,
                Err(error) => {
                    last_error = Some(SessionStoreError::Io(error.to_string()));
                    continue;
                }
            };

            let state: FileStoreData = match serde_json::from_slice(&contents) {
                Ok(state) => state,
                Err(error) => {
                    last_error = Some(SessionStoreError::Serialization(error.to_string()));
                    continue;
                }
            };

            // Restore a copy only after confirming that it is valid.
            if let Err(error) = fs::rename(&backup_path, path) {
                last_error = Some(SessionStoreError::Io(error.to_string()));
                continue;
            }

            return Ok(Some(state));
        }

        match last_error {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }

    /// Gets the current Unix timestamp in milliseconds.
    fn unix_timestamp() -> Result<u64, SessionStoreError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionStoreError::TtlOverflow)?;

        u64::try_from(duration.as_millis()).map_err(|_| SessionStoreError::TtlOverflow)
    }

    /// Calculates the expiration Unix timestamp in milliseconds.
    fn expiration_timestamp(ttl: Duration) -> Result<u64, SessionStoreError> {
        let expiration = SystemTime::now()
            .checked_add(ttl)
            .ok_or(SessionStoreError::TtlOverflow)?;

        let duration = expiration
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionStoreError::TtlOverflow)?;

        u64::try_from(duration.as_millis()).map_err(|_| SessionStoreError::TtlOverflow)
    }

    /// Persists the current state through a temporary file and backup.
    fn persist(&self, data: &FileStoreData) -> Result<(), SessionStoreError> {
        let contents = serde_json::to_vec_pretty(data)
            .map_err(|error| SessionStoreError::Serialization(error.to_string()))?;

        let parent = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty());

        if let Some(parent) = parent {
            fs::create_dir_all(parent).map_err(|error| SessionStoreError::Io(error.to_string()))?;
        }

        let id = uuid::Uuid::new_v4();

        let temp_path = self.path.with_extension(format!("tmp-{id}"));

        let backup_path = self.path.with_extension(format!("backup-{id}"));

        let mut temp_file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
            .map_err(|error| SessionStoreError::Io(error.to_string()))?;

        use std::io::Write;

        if let Err(error) = temp_file.write_all(&contents) {
            let _ = fs::remove_file(&temp_path);
            return Err(SessionStoreError::Io(error.to_string()));
        }

        if let Err(error) = temp_file.sync_all() {
            let _ = fs::remove_file(&temp_path);
            return Err(SessionStoreError::Io(error.to_string()));
        }

        drop(temp_file);

        let has_original = self
            .path
            .try_exists()
            .map_err(|error| SessionStoreError::Io(error.to_string()))?;

        if has_original && let Err(error) = fs::rename(&self.path, &backup_path) {
            let _ = fs::remove_file(&temp_path);
            return Err(SessionStoreError::Io(error.to_string()));
        }

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            let replacement_error = error.to_string();

            if has_original && let Err(restore_error) = fs::rename(&backup_path, &self.path) {
                let _ = fs::remove_file(&temp_path);

                return Err(SessionStoreError::Io(format!(
                    "Failed to replace session file: {replacement_error}. \
                    Failed to restore original file: {restore_error}"
                )));
            }

            let _ = fs::remove_file(&temp_path);

            return Err(SessionStoreError::Io(replacement_error));
        }

        if has_original {
            let _ = fs::remove_file(&backup_path);
        }

        Ok(())
    }

    /// Removes expired sessions and revocations.
    fn cleanup_expired(data: &mut FileStoreData, now: u64) -> usize {
        let expired_ids: Vec<String> = data
            .sessions
            .iter()
            .filter(|(_, entry)| entry.expires_at.is_some_and(|expiry| now >= expiry))
            .map(|(session_id, _)| session_id.clone())
            .collect();

        let removed_sessions = expired_ids.len();

        for session_id in expired_ids {
            data.sessions.remove(&session_id);
            data.revoked.insert(session_id, None);
        }

        let revoked_before = data.revoked.len();

        data.revoked
            .retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        removed_sessions + revoked_before - data.revoked.len()
    }

    /// Gets the data for a session.
    pub fn get_session(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        // Blocks file access between different instances.
        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        // Always gets the latest version from disk.
        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        let removed = Self::cleanup_expired(&mut next_state, now);

        let data = next_state
            .sessions
            .get(session_id)
            .map(|entry| entry.data.clone());

        if removed > 0 {
            self.persist(&next_state)?;
        }

        // Updates the local copy even when no cleanup occurred.
        *state = next_state;

        Ok(data)
    }

    /// Saves or updates a session without an expiration time.
    pub fn set_session(&self, session_id: String, data: Value) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        let removed = Self::cleanup_expired(&mut next_state, now);

        if next_state.revoked.contains_key(&session_id) {
            if removed > 0 {
                self.persist(&next_state)?;
            }

            *state = next_state;

            return Err(SessionStoreError::RevokedSession);
        }

        next_state.sessions.insert(
            session_id,
            FileSessionEntry {
                data,
                expires_at: None,
            },
        );

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Saves a session with a limited time to live.
    pub fn set_session_with_ttl(
        &self,
        session_id: String,
        data: Value,
        ttl: Duration,
    ) -> Result<(), SessionStoreError> {
        let expires_at = Self::expiration_timestamp(ttl)?;

        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        // Blocks file access between different instances.
        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        // Always gets the latest version from disk.
        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        let removed = Self::cleanup_expired(&mut next_state, now);

        if next_state.revoked.contains_key(&session_id) {
            if removed > 0 {
                self.persist(&next_state)?;
            }

            *state = next_state;

            return Err(SessionStoreError::RevokedSession);
        }

        next_state.sessions.insert(
            session_id,
            FileSessionEntry {
                data,
                expires_at: Some(expires_at),
            },
        );

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Removes a session without revoking its identifier.
    pub fn remove_session(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        // Blocks file access between different instances.
        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        // Always gets the latest version from disk.
        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        Self::cleanup_expired(&mut next_state, now);

        next_state.sessions.remove(session_id);

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Permanently revokes a session.
    ///
    /// The revoked identifier cannot be reused.
    pub fn revoke_session(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        // Blocks file access between different instances.
        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        // Always gets the latest version from disk.
        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        Self::cleanup_expired(&mut next_state, now);

        next_state.sessions.remove(session_id);
        next_state.revoked.insert(session_id.to_string(), None);

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Revokes a session for a specified period.
    ///
    /// The minimum revocation TTL is 24 hours.
    /// If a permanent revocation already exists, it is preserved.
    pub fn revoke_session_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        // Blocks file access between different instances.
        let _file_lock = Self::acquire_file_lock(&self.lock_path)?;

        // Always gets the latest version from disk.
        let mut next_state = Self::load_state(&self.path)?;

        let now = Self::unix_timestamp()?;
        Self::cleanup_expired(&mut next_state, now);

        next_state.sessions.remove(session_id);

        let new_expiry = match ttl {
            Some(ttl) => {
                let ttl = ttl.max(REVOCATION_TTL);
                Some(Self::expiration_timestamp(ttl)?)
            }
            None => None,
        };

        match next_state.revoked.get(session_id) {
            Some(None) => {}
            Some(Some(existing_expiry)) => {
                if new_expiry.is_none()
                    || new_expiry.is_some_and(|expiry| expiry > *existing_expiry)
                {
                    next_state
                        .revoked
                        .insert(session_id.to_string(), new_expiry);
                }
            }
            None => {
                next_state
                    .revoked
                    .insert(session_id.to_string(), new_expiry);
            }
        }

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Removes temporary files abandoned for more than 24 hours.
    fn cleanup_temporary_files(path: &Path, cutoff: SystemTime) -> Result<(), SessionStoreError> {
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or(Path::new("."));

        let stem = path
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                SessionStoreError::Io("Nome do ficheiro de sessões inválido.".to_string())
            })?;

        let prefix = format!("{stem}.tmp-");

        let entries = match fs::read_dir(parent) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(());
            }
            Err(error) => {
                return Err(SessionStoreError::Io(error.to_string()));
            }
        };

        for entry in entries {
            let entry = entry.map_err(|error| SessionStoreError::Io(error.to_string()))?;

            if !entry.file_name().to_string_lossy().starts_with(&prefix) {
                continue;
            }

            let file_type = match entry.file_type() {
                Ok(file_type) => file_type,
                Err(_) => continue,
            };

            if !file_type.is_file() {
                continue;
            }

            let modified = match entry.metadata().and_then(|metadata| metadata.modified()) {
                Ok(modified) => modified,
                Err(_) => continue,
            };

            if modified < cutoff {
                fs::remove_file(entry.path())
                    .map_err(|error| SessionStoreError::Io(error.to_string()))?;
            }
        }

        Ok(())
    }
}

/// In-memory session storage.
#[derive(Clone, Default)]
pub struct MemoryStore {
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    revoked: Arc<RwLock<HashMap<String, Option<Instant>>>>,
    last_cleanup: Arc<Mutex<Option<Instant>>>,
}

impl MemoryStore {
    /// Creates an empty session storage.
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

    /// Removes expired sessions and revocation records.
    ///
    /// Returns the total number of removed entries.
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

        // Revoke and remove expired sessions.
        let removed_sessions = Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Remove expired revocation records.
        let revoked_before = revoked.len();

        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        let removed_revocations = revoked_before - revoked.len();

        Ok(removed_sessions + removed_revocations)
    }
    /// Runs cleanup when the configured interval has elapsed.
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
        // Identify expired sessions.
        let expired_ids: Vec<String> = sessions
            .iter()
            .filter(|(_, entry)| entry.expires_at.is_some_and(|expiry| now >= expiry))
            .map(|(session_id, _)| session_id.clone())
            .collect();

        if expired_ids.is_empty() {
            return Ok(0);
        }

        // Permanently record expired IDs as revoked.
        for session_id in &expired_ids {
            revoked
                .entry(session_id.clone())
                .and_modify(|expiry| *expiry = None)
                .or_insert(None);
        }

        // Remove expired sessions.
        sessions.retain(|_, entry| entry.expires_at.is_none_or(|expiry| now < expiry));

        Ok(expired_ids.len())
    }
}

impl SessionStore for FileStore {
    fn get(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError> {
        self.get_session(session_id)
    }

    fn set(&self, session_id: String, data: Value) -> Result<(), SessionStoreError> {
        self.set_session(session_id, data)
    }

    fn set_with_ttl(
        &self,
        session_id: String,
        data: Value,
        ttl: Duration,
    ) -> Result<(), SessionStoreError> {
        self.set_session_with_ttl(session_id, data, ttl)
    }

    fn remove(&self, session_id: &str) -> Result<(), SessionStoreError> {
        self.remove_session(session_id)
    }

    fn revoke(&self, session_id: &str) -> Result<(), SessionStoreError> {
        self.revoke_session(session_id)
    }

    fn revoke_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError> {
        self.revoke_session_with_ttl(session_id, ttl)
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
            // Calculate the deadline before modifying the storage.

            // Preserve the global lock acquisition order.
            let mut revoked = self
                .revoked
                .write()
                .map_err(|_| SessionStoreError::LockPoisoned)?;

            // Modify the maps only after all operations
            // that may return errors have completed.
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

        // Remove revocations that have already expired.
        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        // Revoke and remove expired sessions.
        Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Prevent reuse of revoked IDs.
        if revoked.contains_key(&session_id) {
            return Err(SessionStoreError::RevokedSession);
        }

        // Save the new session.
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

        // Remove revocations that have already expired.
        revoked.retain(|_, expiry| expiry.is_none_or(|expiry| now < expiry));

        // Revoke and remove expired sessions.
        Self::revoke_expired_sessions(&mut sessions, &mut revoked, now)?;

        // Do not allow reuse of a revoked ID.
        if revoked.contains_key(&session_id) {
            return Err(SessionStoreError::RevokedSession);
        }

        // Save the new session.
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

        // The old ID must not recover the previous data.
        assert!(store.get(&session_id).unwrap().is_none());

        // Even if an attempt is made to save data using the same ID,
        // the store must not recover the old session.
        // An attempt to reuse the revoked ID must fail.
        let result = store.set(session_id.clone(), json!({"user_id": 999}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        // The old data remains inaccessible.
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

        // Confirm the permanent revocation.
        assert_eq!(
            store.revoked.read().unwrap().get("expired-session"),
            Some(&None)
        );

        // The identifier cannot be reused.
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

        // Confirm that expiration caused a permanent revocation.
        assert_eq!(
            store.revoked.read().unwrap().get("expired-session"),
            Some(&None)
        );

        assert!(store.get("expired-session").unwrap().is_none());
    }

    #[test]
    fn file_store_persists_sessions_after_reopening() {
        let path =
            std::env::temp_dir().join(format!("rustpress-session-{}.json", uuid::Uuid::new_v4()));

        let session_id = "persistent-session";

        // First instance: save the session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(
                    session_id.to_string(),
                    json!({
                        "username": "fabricio",
                        "role": "admin"
                    }),
                )
                .unwrap();
        }

        // Second instance: retrieve the session from the file.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert_eq!(data["role"], "admin");
        }

        // Clean up the test file.
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_recovers_from_backup_after_interruption() {
        let path =
            std::env::temp_dir().join(format!("rustpress-recovery-{}.json", uuid::Uuid::new_v4()));

        let backup_path = path.with_extension("backup-crash");

        let session_id = "recovered-session";

        // Create and persist a session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        // Simulate an interruption after moving the original to the backup.
        std::fs::rename(&path, &backup_path).unwrap();

        // The constructor should automatically recover the backup.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert!(path.exists());
            assert!(!backup_path.exists());
        }

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(backup_path);
    }

    #[test]
    fn file_store_recovers_from_backup_when_main_file_is_corrupted() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-main-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));

        let backup_path = path.with_extension("backup-valid");
        let session_id = "main-corrupt-session";

        // Create and persist a session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        // Simulate the existence of a valid backup.
        std::fs::rename(&path, &backup_path).unwrap();

        // Create a corrupted main file.
        std::fs::write(&path, b"{ invalid json").unwrap();

        // The storage should recover the backup.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert!(path.exists());
            assert!(!backup_path.exists());
        }

        // Locate the preserved corrupted file.
        let corrupt_prefix = format!("{}.corrupt-", path.file_stem().unwrap().to_string_lossy());

        let corrupt_path = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .find(|candidate| {
                candidate
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with(&corrupt_prefix))
            })
            .expect("O ficheiro corrompido deveria continuar preservado");

        // Confirm that the original content has not been changed.
        assert_eq!(std::fs::read(&corrupt_path).unwrap(), b"{ invalid json");

        // Clean up the test files.
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(backup_path);
        let _ = std::fs::remove_file(corrupt_path);
    }

    #[test]
    fn file_store_ignores_directories_during_backup_recovery() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-directory-{}.json",
            uuid::Uuid::new_v4()
        ));

        let directory_backup = path.with_extension("backup-directory");
        let valid_backup = path.with_extension("backup-valid");

        let session_id = "directory-recovery-session";

        // Create and persist a session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        // Simulate an interruption: move the main file to the backup.
        std::fs::rename(&path, &valid_backup).unwrap();

        // Create a directory with the prefix reserved for backups.
        std::fs::create_dir(&directory_backup).unwrap();

        // The storage should ignore the directory and recover the valid file.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert!(path.exists());
            assert!(!valid_backup.exists());
            assert!(directory_backup.exists());
        }

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(directory_backup);
        let _ = std::fs::remove_file(valid_backup);
    }

    #[test]
    fn file_store_skips_corrupted_backup_during_recovery() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));

        let valid_backup = path.with_extension("backup-valid");
        let corrupted_backup = path.with_extension("backup-corrupted");

        let session_id = "valid-recovered-session";

        // Create a session and save a valid copy.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        std::fs::rename(&path, &valid_backup).unwrap();

        // Ensure that the corrupted copy is newer.
        std::thread::sleep(Duration::from_millis(50));

        std::fs::write(&corrupted_backup, b"{ invalid json").unwrap();

        // The storage should ignore the corrupted copy.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert!(path.exists());
        }

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(valid_backup);
        let _ = std::fs::remove_file(corrupted_backup);
    }

    #[test]
    fn file_store_persists_revocation_after_reopening() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-revocation-{}.json",
            uuid::Uuid::new_v4()
        ));

        let session_id = "revoked-session";

        // First instance: save and revoke the session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store.revoke_session(session_id).unwrap();
        }

        // Second instance: confirm that the revocation was preserved.
        {
            let store = FileStore::new(&path).unwrap();

            let result =
                store.set_session(session_id.to_string(), json!({"username": "another-user"}));

            assert_eq!(result, Err(SessionStoreError::RevokedSession));
            assert!(store.get_session(session_id).unwrap().is_none());
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_expires_sessions_after_reopening() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-expiration-{}.json",
            uuid::Uuid::new_v4()
        ));

        let session_id = "expiring-session";

        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session_with_ttl(
                    session_id.to_string(),
                    json!({"username": "fabricio"}),
                    Duration::from_millis(100),
                )
                .unwrap();
        }

        std::thread::sleep(Duration::from_millis(200));

        {
            let store = FileStore::new(&path).unwrap();

            assert!(store.get_session(session_id).unwrap().is_none());

            let result =
                store.set_session(session_id.to_string(), json!({"username": "another-user"}));

            assert_eq!(result, Err(SessionStoreError::RevokedSession));
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_allows_reusing_session_id_after_removal() {
        let path =
            std::env::temp_dir().join(format!("rustpress-removal-{}.json", uuid::Uuid::new_v4()));

        let session_id = "reusable-session";

        // First instance: create and remove the session.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store.remove_session(session_id).unwrap();

            assert!(store.get_session(session_id).unwrap().is_none());
        }

        // Second instance: reuse the same identifier.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(
                    session_id.to_string(),
                    json!({"username": "novo-utilizador"}),
                )
                .unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "novo-utilizador");
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_preserves_permanent_revocation() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-permanent-revocation-{}.json",
            uuid::Uuid::new_v4()
        ));

        let session_id = "permanently-revoked-session";

        // First instance: revoke permanently.
        {
            let store = FileStore::new(&path).unwrap();

            store.revoke_session(session_id).unwrap();

            // Try to replace the permanent revocation with a temporary one.
            store
                .revoke_session_with_ttl(session_id, Some(Duration::from_secs(60)))
                .unwrap();
        }

        // Second instance: confirm that it remains permanently revoked.
        {
            let store = FileStore::new(&path).unwrap();

            let result = store.set_session(session_id.to_string(), json!({"username": "fabricio"}));

            assert_eq!(result, Err(SessionStoreError::RevokedSession));
            assert!(store.get_session(session_id).unwrap().is_none());
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_independent_instances_preserve_revocations() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-independent-revocation-{}.json",
            uuid::Uuid::new_v4()
        ));

        let store_a = FileStore::new(&path).unwrap();
        let store_b = FileStore::new(&path).unwrap();

        let session_id = "shared-revoked-session";

        // The first instance creates the session.
        store_a
            .set_session(session_id.to_string(), json!({"user": "A"}))
            .unwrap();

        // The second instance permanently revokes the session.
        store_b.revoke_session(session_id).unwrap();

        // The first instance must not be able to reuse the identifier.
        let result = store_a.set_session(session_id.to_string(), json!({"user": "B"}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        assert!(store_a.get_session(session_id).unwrap().is_none());

        // A third instance confirms that the revocation persists.
        let reopened = FileStore::new(&path).unwrap();

        let result = reopened.set_session(session_id.to_string(), json!({"user": "C"}));

        assert_eq!(result, Err(SessionStoreError::RevokedSession));

        drop(reopened);
        drop(store_a);
        drop(store_b);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn file_store_persists_temporary_revocation_after_reopening() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-temporary-revocation-{}.json",
            uuid::Uuid::new_v4()
        ));

        let session_id = "temporarily-revoked-session";

        // First instance: create and revoke temporarily.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store
                .revoke_session_with_ttl(session_id, Some(Duration::from_secs(60)))
                .unwrap();
        }

        // Second instance: confirm that the revocation was preserved.
        {
            let store = FileStore::new(&path).unwrap();

            let result = store.set_session(
                session_id.to_string(),
                json!({"username": "novo-utilizador"}),
            );

            assert_eq!(result, Err(SessionStoreError::RevokedSession));
            assert!(store.get_session(session_id).unwrap().is_none());
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_removes_stale_temporary_files() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-temp-cleanup-{}.json",
            uuid::Uuid::new_v4()
        ));

        let temp_path = path.with_extension("tmp-abandoned");

        std::fs::write(&temp_path, b"abandoned temporary data").unwrap();

        let cutoff = SystemTime::now() + Duration::from_secs(1);

        let store = FileStore::new_with_cutoff(&path, cutoff).unwrap();

        assert!(!temp_path.exists());

        drop(store);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn file_store_preserves_recent_temporary_files() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-temp-recent-{}.json",
            uuid::Uuid::new_v4()
        ));

        let temp_path = path.with_extension("tmp-active");

        std::fs::write(&temp_path, b"temporary data still in use").unwrap();

        // Set a limit of 24 hours in the past.
        let cutoff = SystemTime::now() - Duration::from_secs(24 * 60 * 60);

        let store = FileStore::new_with_cutoff(&path, cutoff).unwrap();

        assert!(temp_path.exists());

        drop(store);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp_path);
    }

    #[test]
    fn file_store_preserves_corrupted_main_file_without_valid_backup() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-no-backup-{}.json",
            uuid::Uuid::new_v4()
        ));

        let corrupted_content = b"{ invalid json";

        // Create a corrupted main file.
        std::fs::write(&path, corrupted_content).unwrap();

        // Try to open the FileStore.
        let result = FileStore::new(&path);

        // It should return a serialization error.
        assert!(matches!(result, Err(SessionStoreError::Serialization(_))));

        // The original file should remain at the main path.
        assert!(path.exists());

        // The original content should remain intact.
        assert_eq!(std::fs::read(&path).unwrap(), corrupted_content);

        // Clean up the test file.
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_preserves_corrupted_main_and_backups_when_all_are_invalid() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-all-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));

        let backup_path = path.with_extension("backup-invalid");
        let main_content = b"{ invalid main json";
        let backup_content = b"{ invalid backup json";

        // Create the corrupted main file.
        std::fs::write(&path, main_content).unwrap();

        // Create a corrupted backup as well.
        std::fs::write(&backup_path, backup_content).unwrap();

        // Try to open the storage.
        let result = FileStore::new(&path);

        // It should return a serialization error.
        assert!(matches!(result, Err(SessionStoreError::Serialization(_))));

        // The original main file should be restored.
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), main_content);

        // The invalid backup should also remain preserved.
        assert!(backup_path.exists());
        assert_eq!(std::fs::read(&backup_path).unwrap(), backup_content);

        // Clean up the test files.
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(backup_path);
    }

    #[test]
    fn file_store_preserves_backup_when_restoration_fails() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-rename-{}.json",
            uuid::Uuid::new_v4()
        ));

        let backup_path = path.with_extension("backup-valid");

        // Create a destination that prevents restoring a file.
        std::fs::create_dir(&path).unwrap();

        // Create a valid JSON backup.
        std::fs::write(
            &backup_path,
            serde_json::to_vec(&FileStoreData::default()).unwrap(),
        )
        .unwrap();

        // Try to recover the backup.
        let result = FileStore::recover_backup(&path);

        // Restoration should fail with an I/O error.
        assert!(matches!(result, Err(SessionStoreError::Io(_))));

        // The backup should remain intact.
        assert!(backup_path.exists());

        // The destination remains a directory.
        assert!(path.is_dir());

        // Clean up the test items.
        let _ = std::fs::remove_file(backup_path);
        let _ = std::fs::remove_dir(path);
    }

    #[test]
    fn file_store_preserves_parent_file_when_directory_creation_fails() {
        let parent = std::env::temp_dir().join(format!(
            "rustpress-persist-parent-failure-{}.tmp",
            uuid::Uuid::new_v4()
        ));

        std::fs::write(&parent, b"parent sentinel").unwrap();

        let path = parent.join("sessions.json");

        let lock_path = path.with_extension("lock");

        let store = FileStore {
            path,
            lock_path,
            state: Mutex::new(FileStoreData::default()),
        };

        let result = store.persist(&FileStoreData::default());

        assert!(matches!(result, Err(SessionStoreError::Io(_))));

        assert!(parent.is_file());
        assert_eq!(std::fs::read(&parent).unwrap(), b"parent sentinel");

        let _ = std::fs::remove_file(parent);
    }

    #[test]
    fn file_store_preserves_sessions_during_concurrent_writes() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-concurrent-store-{}.json",
            uuid::Uuid::new_v4()
        ));

        let store = std::sync::Arc::new(FileStore::new(&path).unwrap());

        let workers = 8;
        let sessions_per_worker = 10;

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(workers));

        let mut handles = Vec::new();

        for worker in 0..workers {
            let store = std::sync::Arc::clone(&store);
            let barrier = std::sync::Arc::clone(&barrier);

            let handle = std::thread::spawn(move || {
                barrier.wait();

                for index in 0..sessions_per_worker {
                    let session_id = format!("worker-{worker}-session-{index}");

                    store
                        .set_session(
                            session_id,
                            serde_json::json!({
                                "worker": worker,
                                "session": index
                            }),
                        )
                        .unwrap();
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        for worker in 0..workers {
            for index in 0..sessions_per_worker {
                let session_id = format!("worker-{worker}-session-{index}");

                let session = store.get_session(&session_id).unwrap();

                assert_eq!(
                    session,
                    Some(serde_json::json!({
                        "worker": worker,
                        "session": index
                    }))
                );
            }
        }

        assert_eq!(store.state.lock().unwrap().sessions.len(), 80);

        drop(store);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_preserves_sessions_during_concurrent_independent_writes() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-multi-instance-store-{}.json",
            uuid::Uuid::new_v4()
        ));

        let workers = 8;
        let sessions_per_worker = 10;

        // Each worker will have its own FileStore instance.
        let stores: Vec<_> = (0..workers)
            .map(|_| std::sync::Arc::new(FileStore::new(&path).unwrap()))
            .collect();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(workers));

        let mut handles = Vec::new();

        for (worker, store) in stores.into_iter().enumerate() {
            let barrier = std::sync::Arc::clone(&barrier);

            let handle = std::thread::spawn(move || {
                barrier.wait();

                for index in 0..sessions_per_worker {
                    let session_id = format!("worker-{worker}-session-{index}");

                    store
                        .set_session(
                            session_id,
                            serde_json::json!({
                                "worker": worker,
                                "session": index
                            }),
                        )
                        .unwrap();
                }
            });

            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Open a new instance to verify the persisted data.
        let reopened = FileStore::new(&path).unwrap();

        for worker in 0..workers {
            for index in 0..sessions_per_worker {
                let session_id = format!("worker-{worker}-session-{index}");

                let session = reopened.get_session(&session_id).unwrap();

                assert_eq!(
                    session,
                    Some(serde_json::json!({
                        "worker": worker,
                        "session": index
                    }))
                );
            }
        }

        assert_eq!(reopened.state.lock().unwrap().sessions.len(), 80);

        drop(reopened);

        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("lock"));
    }

    #[test]
    fn file_store_independent_instances_preserve_each_others_sessions() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-independent-store-{}.json",
            uuid::Uuid::new_v4()
        ));

        let store_a = FileStore::new(&path).unwrap();
        let store_b = FileStore::new(&path).unwrap();

        store_a
            .set_session("session-a".to_string(), json!({"user": "A"}))
            .unwrap();

        store_b
            .set_session("session-b".to_string(), json!({"user": "B"}))
            .unwrap();

        let reopened = FileStore::new(&path).unwrap();

        assert_eq!(
            reopened.get_session("session-a").unwrap(),
            Some(json!({"user": "A"}))
        );

        assert_eq!(
            reopened.get_session("session-b").unwrap(),
            Some(json!({"user": "B"}))
        );
        drop(reopened);
        drop(store_a);
        drop(store_b);

        let _ = std::fs::remove_file(path);
    }
}
