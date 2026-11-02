//! Tipo de erro usado pelos handlers e middlewares do RustPress.
//!
//! [`HandlerError`] transporta uma mensagem e um código HTTP de erro. Por
//! omissão, os erros representam HTTP 500; use [`HandlerError::with_status`]
//! para estados entre 400 e 599.

use std::{error::Error, fmt};

/// Erro devolvido por um handler do RustPress.
#[derive(Debug)]
pub struct HandlerError {
    public_message: String,
    details: Option<String>,
    status: u16,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl HandlerError {
    /// Cria um erro interno do servidor (500).
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            public_message: message.into(),
            details: None,
            status: 500,
            source: None,
        }
    }

    /// Cria um erro com um código HTTP personalizado.
    ///
    /// Apenas aceita códigos entre 400 e 599.
    /// Códigos inválidos são substituídos por 500.
    pub fn with_status(status: u16, message: impl Into<String>) -> Self {
        Self {
            status: if (400..=599).contains(&status) {
                status
            } else {
                500
            },
            ..Self::new(message)
        }
    }

    /// Cria um erro com mensagem pública e detalhes técnicos.
    pub fn with_details(public_message: impl Into<String>, details: impl Into<String>) -> Self {
        Self {
            details: Some(details.into()),
            ..Self::new(public_message)
        }
    }

    /// Cria um erro com código HTTP, mensagem pública e detalhes técnicos.
    pub fn with_status_and_details(
        status: u16,
        public_message: impl Into<String>,
        details: impl Into<String>,
    ) -> Self {
        Self {
            details: Some(details.into()),
            ..Self::with_status(status, public_message)
        }
    }

    /// Associa a causa original ao erro.
    pub fn with_source<E>(mut self, source: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        self.source = Some(Box::new(source));
        self
    }

    /// Devolve o código HTTP associado ao erro.
    pub fn status(&self) -> u16 {
        self.status
    }

    /// Devolve a mensagem pública do erro.
    pub fn public_message(&self) -> &str {
        &self.public_message
    }

    /// Devolve os detalhes técnicos, quando disponíveis.
    pub fn details(&self) -> Option<&str> {
        self.details.as_deref()
    }

    /// Converte um erro externo num HandlerError, preservando a sua causa.
    pub fn from_error<E>(error: E) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        let message = error.to_string();

        Self::new(message).with_source(error)
    }
}

impl fmt::Display for HandlerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.public_message)
    }
}

impl Error for HandlerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

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
        let message = error.to_string();

        Self {
            public_message: message,
            details: None,
            status: 500,
            source: Some(error),
        }
    }
}

impl From<std::io::Error> for HandlerError {
    fn from(error: std::io::Error) -> Self {
        Self::from_error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::HandlerError;
    use std::error::Error;

    #[test]
    fn defaults_to_internal_server_error() {
        let error = HandlerError::new("erro de teste");

        assert_eq!(error.status(), 500);
        assert_eq!(error.to_string(), "erro de teste");
        assert_eq!(error.public_message(), "erro de teste");
        assert_eq!(error.details(), None);
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
    fn creates_error_with_details() {
        let error = HandlerError::with_details(
            "Não foi possível concluir a operação",
            "Falha na ligação à base de dados",
        );

        assert_eq!(
            error.public_message(),
            "Não foi possível concluir a operação"
        );

        assert_eq!(error.details(), Some("Falha na ligação à base de dados"));

        assert_eq!(error.to_string(), "Não foi possível concluir a operação");
    }

    #[test]
    fn creates_error_with_status_and_details() {
        let error = HandlerError::with_status_and_details(
            503,
            "Serviço indisponível",
            "Servidor da base de dados sem resposta",
        );

        assert_eq!(error.status(), 503);
        assert_eq!(error.public_message(), "Serviço indisponível");
        assert_eq!(
            error.details(),
            Some("Servidor da base de dados sem resposta")
        );
    }

    #[test]
    fn converts_external_error_and_preserves_source() {
        let error = std::io::Error::other("erro de teste");
        let handler_error = HandlerError::from_error(error);

        assert_eq!(handler_error.to_string(), "erro de teste");
        assert_eq!(handler_error.status(), 500);

        assert_eq!(handler_error.source().unwrap().to_string(), "erro de teste");
    }

    #[test]
    fn preserves_source_with_custom_public_message() {
        let source = std::io::Error::other("falha técnica");

        let error = HandlerError::new("Ocorreu um erro").with_source(source);

        assert_eq!(error.public_message(), "Ocorreu um erro");
        assert_eq!(error.source().unwrap().to_string(), "falha técnica");
    }

    #[test]
    fn converts_string_error() {
        let error = HandlerError::from("erro externo");

        assert_eq!(error.to_string(), "erro externo");
        assert_eq!(error.status(), 500);
        assert!(error.source().is_none());
    }

    #[test]
    fn converts_io_error_and_preserves_source() {
        let error = std::io::Error::other("erro de I/O");

        let handler_error = HandlerError::from(error);

        assert_eq!(handler_error.to_string(), "erro de I/O");
        assert!(handler_error.source().is_some());
    }
}
