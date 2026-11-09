use std::path::{Path, PathBuf};

use super::UploadError;

/// Configuração dos limites e do armazenamento de ficheiros enviados.
#[derive(Debug, Clone)]
pub struct UploadConfig {
    storage_dir: PathBuf,
    max_total_size: u64,
    max_file_size: u64,
    max_files: usize,
    max_fields: usize,
    max_field_size: u64,
}

impl UploadConfig {
    /// Cria uma configuração com o directório de armazenamento indicado.
    pub fn new<P: Into<PathBuf>>(storage_dir: P) -> Self {
        Self {
            storage_dir: storage_dir.into(),
            ..Self::default()
        }
    }

    /// Define o tamanho máximo total do pedido, em bytes.
    pub fn with_max_total_size(mut self, size: u64) -> Self {
        self.max_total_size = size;
        self
    }

    /// Define o tamanho máximo de cada ficheiro, em bytes.
    pub fn with_max_file_size(mut self, size: u64) -> Self {
        self.max_file_size = size;
        self
    }

    /// Define o número máximo de ficheiros por pedido.
    pub fn with_max_files(mut self, count: usize) -> Self {
        self.max_files = count;
        self
    }

    /// Define o número máximo de campos por pedido.
    pub fn with_max_fields(mut self, count: usize) -> Self {
        self.max_fields = count;
        self
    }

    /// Define o tamanho máximo de cada campo de texto, em bytes.
    pub fn with_max_field_size(mut self, size: u64) -> Self {
        self.max_field_size = size;
        self
    }

    /// Valida os limites e o directório de armazenamento.
    pub fn validate(&self) -> Result<(), UploadError> {
        if self.storage_dir.as_os_str().is_empty() {
            return Err(UploadError::EmptyStorageDirectory);
        }

        if self.max_total_size == 0 {
            return Err(UploadError::InvalidTotalSize);
        }

        if self.max_file_size == 0 || self.max_file_size > self.max_total_size {
            return Err(UploadError::InvalidFileSize);
        }

        if self.max_files == 0 {
            return Err(UploadError::InvalidFileCount);
        }

        if self.max_fields == 0 {
            return Err(UploadError::InvalidFieldCount);
        }

        if self.max_field_size == 0 || self.max_field_size > self.max_total_size {
            return Err(UploadError::InvalidFieldSize);
        }

        Ok(())
    }

    /// Devolve o directório de armazenamento.
    pub fn storage_dir(&self) -> &Path {
        &self.storage_dir
    }

    /// Devolve o tamanho máximo total do pedido, em bytes.
    pub fn max_total_size(&self) -> u64 {
        self.max_total_size
    }

    /// Devolve o tamanho máximo de cada ficheiro, em bytes.
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// Devolve o número máximo de ficheiros por pedido.
    pub fn max_files(&self) -> usize {
        self.max_files
    }

    /// Devolve o número máximo de campos por pedido.
    pub fn max_fields(&self) -> usize {
        self.max_fields
    }

    /// Devolve o tamanho máximo de cada campo de texto, em bytes.
    pub fn max_field_size(&self) -> u64 {
        self.max_field_size
    }
}

impl Default for UploadConfig {
    fn default() -> Self {
        Self {
            storage_dir: PathBuf::from("storage/uploads"),
            max_total_size: 25 * 1024 * 1024,
            max_file_size: 10 * 1024 * 1024,
            max_files: 5,
            max_fields: 32,
            max_field_size: 64 * 1024,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::UploadConfig;
    use crate::upload::UploadError;
    use std::path::Path;

    #[test]
    fn uses_default_configuration() {
        let config = UploadConfig::default();

        assert_eq!(config.storage_dir(), Path::new("storage/uploads"));
        assert_eq!(config.max_total_size(), 25 * 1024 * 1024);
        assert_eq!(config.max_file_size(), 10 * 1024 * 1024);
        assert_eq!(config.max_files(), 5);
        assert_eq!(config.max_fields(), 32);
        assert_eq!(config.max_field_size(), 64 * 1024);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn allows_custom_configuration() {
        let config = UploadConfig::new("uploads")
            .with_max_total_size(50 * 1024 * 1024)
            .with_max_file_size(20 * 1024 * 1024)
            .with_max_files(10)
            .with_max_fields(40)
            .with_max_field_size(128 * 1024);

        assert_eq!(config.storage_dir(), Path::new("uploads"));
        assert_eq!(config.max_total_size(), 50 * 1024 * 1024);
        assert_eq!(config.max_file_size(), 20 * 1024 * 1024);
        assert_eq!(config.max_files(), 10);
        assert_eq!(config.max_fields(), 40);
        assert_eq!(config.max_field_size(), 128 * 1024);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn rejects_empty_storage_directory() {
        let config = UploadConfig::new("");

        assert_eq!(config.validate(), Err(UploadError::EmptyStorageDirectory));
    }

    #[test]
    fn rejects_zero_total_size() {
        let config = UploadConfig::default().with_max_total_size(0);

        assert_eq!(config.validate(), Err(UploadError::InvalidTotalSize));
    }

    #[test]
    fn rejects_file_size_larger_than_total_size() {
        let config = UploadConfig::default()
            .with_max_total_size(1024)
            .with_max_file_size(2048);

        assert_eq!(config.validate(), Err(UploadError::InvalidFileSize));
    }

    #[test]
    fn rejects_zero_file_count() {
        let config = UploadConfig::default().with_max_files(0);

        assert_eq!(config.validate(), Err(UploadError::InvalidFileCount));
    }

    #[test]
    fn rejects_zero_field_count() {
        let config = UploadConfig::default().with_max_fields(0);

        assert_eq!(config.validate(), Err(UploadError::InvalidFieldCount));
    }

    #[test]
    fn rejects_zero_field_size() {
        let config = UploadConfig::default().with_max_field_size(0);

        assert_eq!(config.validate(), Err(UploadError::InvalidFieldSize));
    }

    #[test]
    fn rejects_field_size_larger_than_total_size() {
        let config = UploadConfig::default()
            .with_max_total_size(1024)
            .with_max_file_size(512)
            .with_max_field_size(2048);

        assert_eq!(config.validate(), Err(UploadError::InvalidFieldSize));
    }
}
