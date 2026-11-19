use std::fmt;

/// Errors related to file configuration and receiving uploads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    /// The storage directory was not defined.
    EmptyStorageDirectory,

    /// The maximum total request size is invalid.
    InvalidTotalSize,

    /// The maximum file size is invalid.
    InvalidFileSize,

    /// The maximum number of files is invalid.
    InvalidFileCount,

    /// The maximum number of fields is invalid.
    InvalidFieldCount,

    /// The maximum field size is invalid.
    InvalidFieldSize,

    /// An error occurred while creating, writing, or moving a file.
    Storage(String),

    /// The multipart body could not be processed.
    Multipart(String),

    /// The maximum number of files was exceeded.
    TooManyFiles {
        /// Configured file limit.
        limit: usize,
    },

    /// The maximum size of a file was exceeded.
    FileSizeExceeded {
        /// Configured limit in bytes.
        limit: u64,
    },

    /// The maximum number of fields was exceeded.
    TooManyFields {
        /// Configured field limit.
        limit: usize,
    },

    /// The maximum size of a field was exceeded.
    FieldSizeExceeded {
        /// Configured limit in bytes.
        limit: u64,
    },
}

impl fmt::Display for UploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyStorageDirectory => {
                write!(f, "O directório de armazenamento não pode estar vazio.")
            }
            Self::InvalidTotalSize => {
                write!(
                    f,
                    "O tamanho máximo total do pedido deve ser maior que zero."
                )
            }
            Self::InvalidFileSize => {
                write!(f, "O tamanho máximo de ficheiro deve ser maior que zero.")
            }
            Self::InvalidFileCount => {
                write!(f, "O número máximo de ficheiros deve ser maior que zero.")
            }
            Self::InvalidFieldCount => {
                write!(f, "O número máximo de campos deve ser maior que zero.")
            }
            Self::InvalidFieldSize => {
                write!(f, "O tamanho máximo de campo deve ser maior que zero.")
            }
            Self::Storage(error) => {
                write!(f, "Erro no armazenamento do ficheiro: {error}")
            }
            Self::Multipart(error) => {
                write!(f, "Erro ao processar multipart: {error}")
            }
            Self::TooManyFiles { limit } => {
                write!(f, "O número máximo de ficheiros é {limit}.")
            }
            Self::FileSizeExceeded { limit } => {
                write!(f, "O tamanho máximo de ficheiro é {limit} bytes.")
            }
            Self::TooManyFields { limit } => {
                write!(f, "O número máximo de campos é {limit}.")
            }
            Self::FieldSizeExceeded { limit } => {
                write!(f, "O tamanho máximo de campo é {limit} bytes.")
            }
        }
    }
}

impl std::error::Error for UploadError {}
