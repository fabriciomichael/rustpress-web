use std::fmt;

/// Erros relacionados com a configuração e recepção de ficheiros.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadError {
    /// O directório de armazenamento não foi definido.
    EmptyStorageDirectory,

    /// O tamanho máximo total do pedido é inválido.
    InvalidTotalSize,

    /// O tamanho máximo de ficheiro é inválido.
    InvalidFileSize,

    /// O número máximo de ficheiros é inválido.
    InvalidFileCount,

    /// O número máximo de campos é inválido.
    InvalidFieldCount,

    /// O tamanho máximo de campo é inválido.
    InvalidFieldSize,

    /// Ocorreu um erro ao criar, escrever ou mover um ficheiro.
    Storage(String),

    /// O corpo multipart não pôde ser processado.
    Multipart(String),

    /// O número máximo de ficheiros foi excedido.
    TooManyFiles {
        /// Limite configurado de ficheiros.
        limit: usize,
    },

    /// O tamanho máximo de um ficheiro foi excedido.
    FileSizeExceeded {
        /// Limite configurado em bytes.
        limit: u64,
    },

    /// O número máximo de campos foi excedido.
    TooManyFields {
        /// Limite configurado de campos.
        limit: usize,
    },

    /// O tamanho máximo de um campo foi excedido.
    FieldSizeExceeded {
        /// Limite configurado em bytes.
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
