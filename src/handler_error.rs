use std::{error::Error, fmt};
/// Erro devolvido por um handler do RustPress.
#[derive(Debug)]
pub struct HandlerError {
    message: String,
}

impl HandlerError {
    /// Cria um erro a partir de uma mensagem.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
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
    fn converts_external_error() {
        let error = std::io::Error::other("erro de teste");

        let handler_error = HandlerError::from_error(error);

        assert_eq!(handler_error.to_string(), "erro de teste");
    }

    #[test]
    fn converts_string_error() {
        let error = std::io::Error::other("erro externo");

        let handler_error = HandlerError::from_error(error);

        assert_eq!(handler_error.to_string(), "erro externo");
    }
}
