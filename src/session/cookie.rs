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

    /// Cabeçalho Cookie inválido.
    InvalidHeader,
}

pub(super) fn is_valid_cookie_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

impl std::fmt::Display for CookieError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidName => write!(f, "Nome do cookie inválido"),
            Self::InvalidValue => write!(f, "Valor do cookie inválido"),
            Self::InvalidAttribute => write!(f, "Atributo do cookie inválido"),
            Self::InvalidHeader => write!(f, "Cabeçalho Cookie inválido"),
        }
    }
}

impl std::error::Error for CookieError {}

impl Cookie {
    /// Cria um novo cookie.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Procura um cookie pelo nome num cabeçalho HTTP `Cookie`.
    pub fn get(header: &str, name: &str) -> Option<String> {
        Self::parse(header)
            .into_iter()
            .find(|cookie| cookie.name == name)
            .map(|cookie| cookie.value)
    }

    /// Procura um cookie pelo nome, rejeitando ocorrências duplicadas.
    pub fn get_unique(header: &str, name: &str) -> Result<Option<String>, CookieError> {
        if !is_valid_cookie_name(name) {
            return Err(CookieError::InvalidName);
        }
        let mut result = None;

        for part in header.split(';') {
            let part = part.trim();

            let (cookie_name, value) = match part.split_once('=') {
                Some((name, value)) => (name.trim(), value),
                None if part == name => {
                    return Err(CookieError::InvalidValue);
                }
                None => continue,
            };

            let cookie_name = cookie_name.trim();

            if cookie_name != name {
                continue;
            }

            if result.is_some() {
                return Err(CookieError::InvalidValue);
            }

            let value = value.trim();

            let value = if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
                &value[1..value.len() - 1]
            } else {
                value
            };

            if value.is_empty()
                || !value.bytes().all(|byte| {
                    byte == 0x21
                        || (0x23..=0x2B).contains(&byte)
                        || (0x2D..=0x3A).contains(&byte)
                        || (0x3C..=0x5B).contains(&byte)
                        || (0x5D..=0x7E).contains(&byte)
                })
            {
                return Err(CookieError::InvalidValue);
            }

            result = Some(value.to_string());
        }

        Ok(result)
    }

    /// Procura um cookie em vários cabeçalhos, rejeitando ocorrências duplicadas.
    pub fn get_unique_from_headers<'a, I>(
        headers: I,
        name: &str,
    ) -> Result<Option<String>, CookieError>
    where
        I: IntoIterator<Item = &'a str>,
    {
        Self::get_unique("", name)?;

        let mut result = None;

        for header in headers {
            if let Some(value) = Self::get_unique(header, name)? {
                if result.is_some() {
                    return Err(CookieError::InvalidValue);
                }

                result = Some(value);
            }
        }

        Ok(result)
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

        config.validate()?;

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

    #[test]
    fn gets_unique_cookie_by_name() {
        let result = Cookie::get_unique("user=123; session=abc123; theme=dark", "session");

        assert_eq!(result, Ok(Some("abc123".to_string())));
    }

    #[test]
    fn rejects_duplicate_session_cookies() {
        let result = Cookie::get_unique("session=abc123; theme=dark; session=xyz789", "session");

        assert_eq!(result, Err(CookieError::InvalidValue));
    }

    #[test]
    fn rejects_empty_session_cookie() {
        let result = Cookie::get_unique("theme=dark; session=", "session");

        assert_eq!(result, Err(CookieError::InvalidValue));
    }

    #[test]
    fn ignores_malformed_unrelated_cookies() {
        let result = Cookie::get_unique("theme=dark; malformed-cookie; session=abc123", "session");

        assert_eq!(result, Ok(Some("abc123".to_string())));
    }

    #[test]
    fn returns_none_when_session_cookie_is_missing() {
        let result = Cookie::get_unique("user=123; theme=dark", "session");

        assert_eq!(result, Ok(None));
    }

    #[test]
    fn rejects_session_cookie_without_equals_sign() {
        let result = Cookie::get_unique("theme=dark; session; user=123", "session");

        assert_eq!(result, Err(CookieError::InvalidValue));
    }

    #[test]
    fn get_unique_from_headers_finds_cookie() {
        let headers = ["theme=dark; rustpress_session=abc123"];

        let result = Cookie::get_unique_from_headers(headers, "rustpress_session");

        assert_eq!(result.unwrap(), Some("abc123".to_string()));
    }

    #[test]
    fn get_unique_from_headers_rejects_duplicates() {
        let headers = ["rustpress_session=abc123", "rustpress_session=xyz789"];

        let result = Cookie::get_unique_from_headers(headers, "rustpress_session");

        assert!(result.is_err());
    }

    #[test]
    fn get_unique_from_headers_ignores_unrelated_cookies() {
        let headers = ["theme=dark", "language=pt", "rustpress_session=abc123"];

        let result = Cookie::get_unique_from_headers(headers, "rustpress_session");

        assert_eq!(result.unwrap(), Some("abc123".to_string()));
    }

    #[test]
    fn get_unique_from_headers_returns_none_when_cookie_is_missing() {
        let headers = ["theme=dark", "language=pt"];

        let result = Cookie::get_unique_from_headers(headers, "rustpress_session");

        assert_eq!(result.unwrap(), None);
    }

    #[test]
    fn get_unique_from_headers_rejects_invalid_cookie() {
        let headers = ["rustpress_session="];

        let result = Cookie::get_unique_from_headers(headers, "rustpress_session");

        assert!(result.is_err());
    }

    #[test]
    fn gets_unique_quoted_cookie() {
        let result = Cookie::get_unique("session=\"abc123\"", "session");

        assert_eq!(result, Ok(Some("abc123".to_string())));
    }

    #[test]
    fn rejects_invalid_name_in_get_unique() {
        let result = Cookie::get_unique("session=abc123", "session;admin");

        assert_eq!(result, Err(CookieError::InvalidName));
    }

    #[test]
    fn get_unique_from_headers_rejects_invalid_name_without_headers() {
        let headers: [&str; 0] = [];

        let result = Cookie::get_unique_from_headers(headers, "session;admin");

        assert_eq!(result, Err(CookieError::InvalidName));
    }
}
