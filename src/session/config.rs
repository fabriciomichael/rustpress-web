/// Configuração do atributo SameSite de um cookie.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SameSite {
    /// O cookie só é enviado em pedidos do mesmo site.
    Strict,

    /// Permite o envio em determinadas navegações entre sites.
    Lax,

    /// Permite o envio em pedidos entre sites.
    /// Exige que o atributo Secure esteja activo.
    None,
}

/// Configuração das sessões e do respectivo cookie.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Nome do cookie que identifica a sessão.
    pub cookie_name: String,

    /// Caminho no qual o cookie é válido.
    pub path: String,

    /// Domínio no qual o cookie é válido.
    pub domain: Option<String>,

    /// Tempo de vida da sessão, em segundos.
    pub max_age: u64,

    /// Impede o acesso ao cookie através de JavaScript.
    pub http_only: bool,

    /// Exige uma ligação HTTPS para enviar o cookie.
    pub secure: bool,

    /// Política de envio do cookie entre sites.
    pub same_site: SameSite,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            cookie_name: "rustpress_session".to_string(),
            path: "/".to_string(),
            domain: None,
            max_age: 3600,
            http_only: true,
            secure: false,
            same_site: SameSite::Lax,
        }
    }
}

impl SessionConfig {
    /// Valida as configurações da sessão e do respectivo cookie.
    pub fn validate(&self) -> Result<(), super::cookie::CookieError> {
        use super::cookie::{CookieError, is_valid_cookie_name};

        // Validar o nome do cookie.
        if !is_valid_cookie_name(&self.cookie_name) {
            return Err(CookieError::InvalidName);
        }

        // O caminho deve começar por "/" e não pode conter
        // caracteres de controlo nem ponto e vírgula.
        if !self.path.starts_with('/')
            || !self.path.bytes().all(|byte| {
                (0x20..=0x7E).contains(&byte) && !matches!(byte, b';' | b',' | b'"' | b'\\')
            })
        {
            return Err(CookieError::InvalidAttribute);
        }

        // Validar o domínio, quando definido.
        if let Some(domain) = &self.domain {
            let valid_domain = !domain.is_empty()
                && domain.len() <= 253
                && domain.contains('.')
                && domain.split('.').all(|label| {
                    !label.is_empty()
                        && label.len() <= 63
                        && !label.starts_with('-')
                        && !label.ends_with('-')
                        && label
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
                })
                && domain
                    .rsplit('.')
                    .next()
                    .is_some_and(|label| label.bytes().any(|byte| byte.is_ascii_alphabetic()));

            if !valid_domain {
                return Err(CookieError::InvalidAttribute);
            }
        }

        // SameSite=None exige Secure.
        if self.same_site == SameSite::None && !self.secure {
            return Err(CookieError::InvalidAttribute);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_default_config() {
        assert!(SessionConfig::default().validate().is_ok());
    }

    #[test]
    fn rejects_empty_cookie_name() {
        let config = SessionConfig {
            cookie_name: String::new(),
            ..SessionConfig::default()
        };

        assert_eq!(
            config.validate(),
            Err(super::super::cookie::CookieError::InvalidName)
        );
    }

    #[test]
    fn rejects_path_without_leading_slash() {
        let config = SessionConfig {
            path: "sessions".to_string(),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_invalid_domain() {
        let config = SessionConfig {
            domain: Some("-example.com".to_string()),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_samesite_none_without_secure() {
        let config = SessionConfig {
            same_site: SameSite::None,
            secure: false,
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_domain_with_label_too_long() {
        let domain = format!("{}.com", "a".repeat(64));

        let config = SessionConfig {
            domain: Some(domain),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn accepts_samesite_none_with_secure() {
        let config = SessionConfig {
            same_site: SameSite::None,
            secure: true,
            ..SessionConfig::default()
        };

        assert!(config.validate().is_ok());
    }
    #[test]
    fn rejects_domain_with_empty_label() {
        let config = SessionConfig {
            domain: Some("example..com".to_string()),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_domain_without_dot() {
        let config = SessionConfig {
            domain: Some("localhost".to_string()),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_domain_with_numeric_tld() {
        let config = SessionConfig {
            domain: Some("example.123".to_string()),
            ..SessionConfig::default()
        };

        assert!(config.validate().is_err());
    }
}
