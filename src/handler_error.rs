use std::{error::Error, fmt};

/// Erro devolvido por um handler do RustPress.
#[derive(Debug)]
pub struct HandlerError {
    message: String,
    status: u16,
}

impl HandlerError {
    /// Cria um erro interno do servidor (500).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: 500,
        }
    }

    /// Cria um erro com um código HTTP personalizado.
    ///
    /// Apenas aceita códigos de erro entre 400 e 599.
    /// Códigos inválidos são substituídos por 500.
    pub fn with_status(status: u16, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            status: if (400..=599).contains(&status) {
                status
            } else {
                500
            },
        }
    }

    /// Devolve o código HTTP associado ao erro.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Converte um erro externo num HandlerError.
    pub fn from_error<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self::new(error.to_string())
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for HandlerError {}

impl From<String> for HandlerError {
    fn from(error: String) -> Self {
        Self::new(error)
    }
}

impl From<&str> for HandlerError {
    fn from(error: &str) -> Self {
        Self::new(error)
    }
}

impl From<Box<dyn Error + Send + Sync>> for HandlerError {
    fn from(error: Box<dyn Error + Send + Sync>) -> Self {
        Self::new(error.to_string())
    }
}

impl From<std::io::Error> for HandlerError {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::HandlerError;

    #[test]
    fn defaults_to_internal_server_error() {
        let error = HandlerError::new("erro de teste");

        assert_eq!(error.status(), 500);
        assert_eq!(error.to_string(), "erro de teste");
    }

    #[test]
    fn accepts_custom_http_error_status() {
        let error = HandlerError::with_status(404, "recurso não encontrado");

        assert_eq!(error.status(), 404);
        assert_eq!(error.to_string(), "recurso não encontrado");
    }

    #[test]
    fn invalid_status_defaults_to_500() {
        let error = HandlerError::with_status(200, "erro de teste");

        assert_eq!(error.status(), 500);
    }

    #[test]
    fn converts_external_error() {
        let error = std::io::Error::other("erro de teste");
        let handler_error = HandlerError::from_error(error);

        assert_eq!(handler_error.to_string(), "erro de teste");
        assert_eq!(handler_error.status(), 500);
    }

    #[test]
    fn converts_string_error() {
        let handler_error = HandlerError::from("erro externo");

        assert_eq!(handler_error.to_string(), "erro externo");
        assert_eq!(handler_error.status(), 500);
    }
}
