//! Armazenamento interno dos dados das sessões.

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

/// Erros que podem ocorrer durante operações de armazenamento de sessões.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStoreError {
    /// O lock interno do armazenamento foi envenenado.
    LockPoisoned,

    /// O tempo de vida não pode ser representado pelo relógio interno.
    TtlOverflow,

    /// A tentativa de escrita utilizou um ID de sessão revogado.
    RevokedSession,

    /// Ocorreu um erro durante uma operação de ficheiro.
    Io(String),

    /// Ocorreu um erro ao serializar ou desserializar os dados.
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

/// Entrada de sessão preparada para persistência em ficheiro.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FileSessionEntry {
    data: Value,
    expires_at: Option<u64>,
}

/// Estado completo persistido pelo FileStore.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct FileStoreData {
    sessions: HashMap<String, FileSessionEntry>,
    revoked: HashMap<String, Option<u64>>,
}

/// Armazenamento persistente de sessões em ficheiro JSON.
pub struct FileStore {
    path: PathBuf,
    state: Mutex<FileStoreData>,
}

impl FileStore {
    /// Abre um armazenamento persistente de sessões.
    ///
    /// Se o ficheiro principal não existir, tenta recuperar uma cópia
    /// de segurança deixada por uma interrupção anterior.
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

        Self::cleanup_temporary_files(&path, cutoff)?;

        let state = match fs::read(&path) {
            Ok(contents) => serde_json::from_slice(&contents)
                .map_err(|error| SessionStoreError::Serialization(error.to_string()))?,

            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                Self::recover_backup(&path)?.unwrap_or_default()
            }

            Err(error) => {
                return Err(SessionStoreError::Io(error.to_string()));
            }
        };

        Ok(Self {
            path,
            state: Mutex::new(state),
        })
    }
    /// Procura e recupera uma cópia de segurança.
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

            if entry.file_name().to_string_lossy().starts_with(&prefix) {
                backups.push(entry.path());
            }
        }

        // Prioriza as cópias de segurança mais recentes.
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

            // Só restaura uma cópia depois de confirmar que é válida.
            fs::rename(&backup_path, path)
                .map_err(|error| SessionStoreError::Io(error.to_string()))?;

            return Ok(Some(state));
        }

        match last_error {
            Some(error) => Err(error),
            None => Ok(None),
        }
    }

    /// Obtém o timestamp Unix actual em milissegundos.
    fn unix_timestamp() -> Result<u64, SessionStoreError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionStoreError::TtlOverflow)?;

        u64::try_from(duration.as_millis()).map_err(|_| SessionStoreError::TtlOverflow)
    }

    /// Calcula o timestamp Unix de expiração em milissegundos.
    fn expiration_timestamp(ttl: Duration) -> Result<u64, SessionStoreError> {
        let expiration = SystemTime::now()
            .checked_add(ttl)
            .ok_or(SessionStoreError::TtlOverflow)?;

        let duration = expiration
            .duration_since(UNIX_EPOCH)
            .map_err(|_| SessionStoreError::TtlOverflow)?;

        u64::try_from(duration.as_millis()).map_err(|_| SessionStoreError::TtlOverflow)
    }

    /// Persiste o estado actual através de um ficheiro temporário e cópia de segurança.
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

        let has_original = self.path.exists();

        if has_original {
            if let Err(error) = fs::rename(&self.path, &backup_path) {
                let _ = fs::remove_file(&temp_path);
                return Err(SessionStoreError::Io(error.to_string()));
            }
        }

        if let Err(error) = fs::rename(&temp_path, &self.path) {
            let replacement_error = error.to_string();

            if has_original {
                if let Err(restore_error) = fs::rename(&backup_path, &self.path) {
                    let _ = fs::remove_file(&temp_path);

                    return Err(SessionStoreError::Io(format!(
                        "Failed to replace session file: {replacement_error}. \
                         Failed to restore original file: {restore_error}"
                    )));
                }
            }

            let _ = fs::remove_file(&temp_path);

            return Err(SessionStoreError::Io(replacement_error));
        }

        if has_original {
            let _ = fs::remove_file(&backup_path);
        }

        Ok(())
    }

    /// Remove sessões e revogações expiradas.
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

    /// Obtém os dados de uma sessão.
    pub fn get_session(&self, session_id: &str) -> Result<Option<Value>, SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut next_state = state.clone();

        let now = Self::unix_timestamp()?;
        let removed = Self::cleanup_expired(&mut next_state, now);

        let data = next_state
            .sessions
            .get(session_id)
            .map(|entry| entry.data.clone());

        if removed > 0 {
            self.persist(&next_state)?;
            *state = next_state;
        }

        Ok(data)
    }

    /// Guarda ou actualiza uma sessão sem tempo de expiração.
    pub fn set_session(&self, session_id: String, data: Value) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut next_state = state.clone();
        let now = Self::unix_timestamp()?;

        let removed = Self::cleanup_expired(&mut next_state, now);

        // Persiste a limpeza antes de verificar o novo identificador.
        if removed > 0 {
            self.persist(&next_state)?;
            *state = next_state.clone();
        }

        // Impede a reutilização de identificadores revogados.
        if next_state.revoked.contains_key(&session_id) {
            return Err(SessionStoreError::RevokedSession);
        }

        next_state.sessions.insert(
            session_id,
            FileSessionEntry {
                data,
                expires_at: None,
            },
        );

        // Só actualiza o estado activo depois de persistir com sucesso.
        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Guarda uma sessão com um tempo de vida limitado.
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

        let mut next_state = state.clone();
        let now = Self::unix_timestamp()?;

        let removed = Self::cleanup_expired(&mut next_state, now);

        if removed > 0 {
            self.persist(&next_state)?;
            *state = next_state.clone();
        }

        if next_state.revoked.contains_key(&session_id) {
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

    /// Remove uma sessão sem revogar o seu identificador.
    pub fn remove_session(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut next_state = state.clone();

        let now = Self::unix_timestamp()?;
        Self::cleanup_expired(&mut next_state, now);

        next_state.sessions.remove(session_id);

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Revoga permanentemente uma sessão.
    ///
    /// O identificador revogado não poderá ser reutilizado.
    pub fn revoke_session(&self, session_id: &str) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut next_state = state.clone();

        let now = Self::unix_timestamp()?;
        Self::cleanup_expired(&mut next_state, now);

        next_state.sessions.remove(session_id);
        next_state.revoked.insert(session_id.to_string(), None);

        self.persist(&next_state)?;
        *state = next_state;

        Ok(())
    }

    /// Revoga uma sessão durante um período determinado.
    ///
    /// O TTL mínimo de revogação é de 24 horas.
    /// Se já existir uma revogação permanente, ela será preservada.
    pub fn revoke_session_with_ttl(
        &self,
        session_id: &str,
        ttl: Option<Duration>,
    ) -> Result<(), SessionStoreError> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| SessionStoreError::LockPoisoned)?;

        let mut next_state = state.clone();

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

    /// Remove ficheiros temporários abandonados há mais de 24 horas.
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

    #[test]
    fn file_store_persists_sessions_after_reopening() {
        let path =
            std::env::temp_dir().join(format!("rustpress-session-{}.json", uuid::Uuid::new_v4()));

        let session_id = "persistent-session";

        // Primeira instância: guardar a sessão.
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

        // Segunda instância: recuperar a sessão do ficheiro.
        {
            let store = FileStore::new(&path).unwrap();

            let data = store.get_session(session_id).unwrap().unwrap();

            assert_eq!(data["username"], "fabricio");
            assert_eq!(data["role"], "admin");
        }

        // Limpar o ficheiro de teste.
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_recovers_from_backup_after_interruption() {
        let path =
            std::env::temp_dir().join(format!("rustpress-recovery-{}.json", uuid::Uuid::new_v4()));

        let backup_path = path.with_extension("backup-crash");

        let session_id = "recovered-session";

        // Criar e persistir uma sessão.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        // Simular uma interrupção após mover o original para backup.
        std::fs::rename(&path, &backup_path).unwrap();

        // O construtor deverá recuperar automaticamente o backup.
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
    fn file_store_skips_corrupted_backup_during_recovery() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-recovery-corrupt-{}.json",
            uuid::Uuid::new_v4()
        ));

        let valid_backup = path.with_extension("backup-valid");
        let corrupted_backup = path.with_extension("backup-corrupted");

        let session_id = "valid-recovered-session";

        // Criar uma sessão e guardar uma cópia válida.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();
        }

        std::fs::rename(&path, &valid_backup).unwrap();

        // Garantir que a cópia corrompida é mais recente.
        std::thread::sleep(Duration::from_millis(50));

        std::fs::write(&corrupted_backup, b"{ invalid json").unwrap();

        // O armazenamento deve ignorar a cópia corrompida.
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

        // Primeira instância: guardar e revogar a sessão.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store.revoke_session(session_id).unwrap();
        }

        // Segunda instância: confirmar que a revogação foi preservada.
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

        // Primeira instância: criar e remover a sessão.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store.remove_session(session_id).unwrap();

            assert!(store.get_session(session_id).unwrap().is_none());
        }

        // Segunda instância: reutilizar o mesmo identificador.
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

        // Primeira instância: revogar permanentemente.
        {
            let store = FileStore::new(&path).unwrap();

            store.revoke_session(session_id).unwrap();

            // Tentar substituir a revogação permanente por temporária.
            store
                .revoke_session_with_ttl(session_id, Some(Duration::from_secs(60)))
                .unwrap();
        }

        // Segunda instância: confirmar que continua permanentemente revogada.
        {
            let store = FileStore::new(&path).unwrap();

            let result = store.set_session(session_id.to_string(), json!({"username": "fabricio"}));

            assert_eq!(result, Err(SessionStoreError::RevokedSession));
            assert!(store.get_session(session_id).unwrap().is_none());
        }

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn file_store_persists_temporary_revocation_after_reopening() {
        let path = std::env::temp_dir().join(format!(
            "rustpress-temporary-revocation-{}.json",
            uuid::Uuid::new_v4()
        ));

        let session_id = "temporarily-revoked-session";

        // Primeira instância: criar e revogar temporariamente.
        {
            let store = FileStore::new(&path).unwrap();

            store
                .set_session(session_id.to_string(), json!({"username": "fabricio"}))
                .unwrap();

            store
                .revoke_session_with_ttl(session_id, Some(Duration::from_secs(60)))
                .unwrap();
        }

        // Segunda instância: confirmar que a revogação foi preservada.
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

        // Define um limite de 24 horas atrás.
        let cutoff = SystemTime::now() - Duration::from_secs(24 * 60 * 60);

        let store = FileStore::new_with_cutoff(&path, cutoff).unwrap();

        assert!(temp_path.exists());

        drop(store);

        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(temp_path);
    }
}
