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
