//! Representação e leitura de cookies HTTP.

/// Representa um cookie recebido num pedido HTTP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cookie {
    /// Nome do cookie.
    pub name: String,

    /// Valor do cookie.
    pub value: String,
}

/// Erros encontrados durante a criação de um cabeçalho Set-Cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CookieError {
    /// Nome do cookie inválido.
    InvalidName,

    /// Valor do cookie inválido.
    InvalidValue,

    /// Caminho ou domínio inválido.
    InvalidAttribute,
}

impl Cookie {
    /// Cria um novo cookie.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Procura um cookie pelo nome num cabeçalho HTTP `Cookie`.
    pub fn get<'a>(header: &'a str, name: &str) -> Option<String> {
        Self::parse(header)
            .into_iter()
            .find(|cookie| cookie.name == name)
            .map(|cookie| cookie.value)
    }

    /// Interpreta um cabeçalho HTTP `Cookie`.
    ///
    /// Um cabeçalho pode conter vários cookies separados por ponto e vírgula.
    pub fn parse(header: &str) -> Vec<Self> {
        header
            .split(';')
            .filter_map(|part| {
                let (name, value) = part.trim().split_once('=')?;

                let name = name.trim();
                let value = value.trim();

                if name.is_empty() {
                    return None;
                }

                let value = if value.len() >= 2 && value.starts_with('"') && value.ends_with('"') {
                    &value[1..value.len() - 1]
                } else {
                    value
                };

                Some(Self::new(name, value))
            })
            .collect()
    }

    /// Gera o cabeçalho HTTP Set-Cookie usando a configuração da sessão.
    pub fn to_set_cookie(
        &self,
        config: &super::config::SessionConfig,
    ) -> Result<String, CookieError> {
        let valid_name = !self.name.is_empty()
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte));

        if !valid_name {
            return Err(CookieError::InvalidName);
        }

        let valid_value = self.value.bytes().all(|byte| {
            byte == 0x21
                || (0x23..=0x2B).contains(&byte)
                || (0x2D..=0x3A).contains(&byte)
                || (0x3C..=0x5B).contains(&byte)
                || (0x5D..=0x7E).contains(&byte)
        });

        if !valid_value {
            return Err(CookieError::InvalidValue);
        }

        if config.path.is_empty()
            || config.path.contains(';')
            || config.path.bytes().any(|b| b.is_ascii_control())
        {
            return Err(CookieError::InvalidAttribute);
        }

        if let Some(domain) = &config.domain {
            if domain.is_empty()
                || !domain
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-')
            {
                return Err(CookieError::InvalidAttribute);
            }
        }

        if config.same_site == super::config::SameSite::None && !config.secure {
            return Err(CookieError::InvalidAttribute);
        }

        let mut header = format!(
            "{}={}; Path={}; Max-Age={}",
            self.name, self.value, config.path, config.max_age
        );

        if let Some(domain) = &config.domain {
            header.push_str(&format!("; Domain={domain}"));
        }

        if config.http_only {
            header.push_str("; HttpOnly");
        }

        if config.secure {
            header.push_str("; Secure");
        }

        match config.same_site {
            super::config::SameSite::Strict => header.push_str("; SameSite=Strict"),
            super::config::SameSite::Lax => header.push_str("; SameSite=Lax"),
            super::config::SameSite::None => header.push_str("; SameSite=None"),
        }

        Ok(header)
    }
}

#[cfg(test)]
mod tests {
    use super::Cookie;

    #[test]
    fn parses_multiple_cookies() {
        let cookies = Cookie::parse("user=Fabrício; theme=dark");

        assert_eq!(cookies.len(), 2);
        assert_eq!(cookies[0].name, "user");
        assert_eq!(cookies[0].value, "Fabrício");
        assert_eq!(cookies[1].name, "theme");
        assert_eq!(cookies[1].value, "dark");
    }

    #[test]
    fn gets_cookie_by_name() {
        let value = Cookie::get("user=123; theme=dark", "theme");

        assert_eq!(value, Some("dark".to_string()));
    }

    #[test]
    fn returns_none_for_missing_cookie() {
        let value = Cookie::get("user=123", "session");

        assert_eq!(value, None);
    }

    use crate::session::{CookieError, SameSite, SessionConfig};

    #[test]
    fn generates_set_cookie_with_default_config() {
        let cookie = Cookie::new("session_id", "abc123");
        let config = SessionConfig::default();

        let header = cookie.to_set_cookie(&config).unwrap();

        assert_eq!(
            header,
            "session_id=abc123; Path=/; Max-Age=3600; HttpOnly; SameSite=Lax"
        );
    }

    #[test]
    fn generates_secure_cookie() {
        let cookie = Cookie::new("session_id", "abc123");

        let config = SessionConfig {
            secure: true,
            same_site: SameSite::None,
            ..SessionConfig::default()
        };

        let header = cookie.to_set_cookie(&config).unwrap();

        assert!(header.contains("; Secure"));
        assert!(header.contains("; SameSite=None"));
    }

    #[test]
    fn rejects_invalid_cookie_name() {
        let cookie = Cookie::new("session;id", "abc123");
        let config = SessionConfig::default();

        assert_eq!(cookie.to_set_cookie(&config), Err(CookieError::InvalidName));
    }

    #[test]
    fn rejects_invalid_cookie_value() {
        let cookie = Cookie::new("session_id", "abc\n123");
        let config = SessionConfig::default();

        assert_eq!(
            cookie.to_set_cookie(&config),
            Err(CookieError::InvalidValue)
        );
    }

    #[test]
    fn rejects_samesite_none_without_secure() {
        let cookie = Cookie::new("session_id", "abc123");

        let config = SessionConfig {
            same_site: SameSite::None,
            secure: false,
            ..SessionConfig::default()
        };

        assert_eq!(
            cookie.to_set_cookie(&config),
            Err(CookieError::InvalidAttribute)
        );
    }

    #[test]
    fn includes_domain_in_set_cookie() {
        let cookie = Cookie::new("session_id", "abc123");

        let config = SessionConfig {
            domain: Some("example.com".to_string()),
            ..SessionConfig::default()
        };

        let header = cookie.to_set_cookie(&config).unwrap();

        assert!(header.contains("; Domain=example.com"));
    }
}
