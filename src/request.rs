//! Abstracções de leitura dos pedidos HTTP.
//!
//! [`Request`] fornece acesso ao método, caminho, cabeçalhos, parâmetros de
//! rota, query string e corpo HTTP.

use crate::session::{Cookie, CookieError, Session, SessionManager};
use crate::upload::UploadedFile;
use bytes::Bytes;
use hyper::{
    header::{COOKIE, HeaderValue},
    http::Method,
};
use std::sync::{Arc, Mutex};

/// Representa uma requisição HTTP recebida pelo servidor.
///
/// Esta estrutura encapsula a mensagem original do Hyper e acrescenta
/// informações úteis para a aplicação web, nomeadamente os parâmetros de rota
/// extraídos pelo router. A intenção é fornecer uma API simples e idiomática
/// para os handlers registados nas rotas.
pub struct Request {
    inner: hyper::Request<Bytes>,
    params: Vec<(String, String)>,
    session: Option<Arc<Mutex<Session>>>,
    session_manager: Option<Arc<SessionManager>>,
    multipart_fields: Vec<(String, String)>,
    is_multipart: bool,
    uploaded_files: Vec<UploadedFile>,
}

impl Request {
    /// Cria uma nova instância de `Request` com a requisição HTTP original e os
    /// parâmetros de rota já resolvidos.
    pub(crate) fn new(inner: hyper::Request<Bytes>, params: Vec<(String, String)>) -> Self {
        Self {
            inner,
            params,
            session: None,
            session_manager: None,
            multipart_fields: Vec::new(),
            is_multipart: false,
            uploaded_files: Vec::new(),
        }
    }

    /// Devolve a sessão associada à requisição.
    ///
    /// Retorna `None` quando a gestão de sessões não está activada.
    pub fn session(&self) -> Option<Arc<Mutex<Session>>> {
        self.session.clone()
    }

    /// Associa uma sessão ao pedido HTTP.
    pub(crate) fn set_session(&mut self, session: Arc<Mutex<Session>>) {
        self.session = Some(session);
    }

    /// Associa o gestor de sessões ao pedido HTTP.
    pub(crate) fn set_session_manager(&mut self, manager: Arc<SessionManager>) {
        self.session_manager = Some(manager);
    }

    /// Regenera o identificador da sessão actual.
    ///
    /// Mantém os dados existentes, mas substitui o identificador
    /// por um novo UUID.
    ///
    /// Retorna `false` se as sessões não estiverem activadas
    /// ou se não for possível bloquear a sessão.
    pub fn regenerate_session(&self) -> Result<bool, crate::session::SessionError> {
        let (Some(session), Some(manager)) = (&self.session, &self.session_manager) else {
            return Ok(false);
        };

        let mut session = session
            .lock()
            .map_err(|_| crate::session::SessionError::LockPoisoned)?;

        manager.regenerate(&mut session)?;

        Ok(true)
    }

    /// Devolve o método HTTP utilizado na requisição, como `GET`, `POST`,
    /// `PUT`, `PATCH` ou `DELETE`.
    pub fn method(&self) -> &Method {
        self.inner.method()
    }

    /// Devolve o caminho da URL da requisição sem incluir a query string.
    ///
    /// Por exemplo, para `/utilizadores/42?activo=true`, este método devolve
    /// `/utilizadores/42`.
    pub fn path(&self) -> &str {
        self.inner.uri().path()
    }

    /// Procura um cabeçalho HTTP específico e devolve o valor correspondente,
    /// caso exista.
    pub fn header(&self, name: &str) -> Option<&HeaderValue> {
        self.inner.headers().get(name)
    }

    /// Devolve todos os cookies recebidos na requisição.
    ///
    /// Suporta vários cabeçalhos Cookie.
    /// Retorna erro se algum cabeçalho não puder ser interpretado como texto.
    pub fn cookies(&self) -> Result<Vec<Cookie>, CookieError> {
        let mut cookies = Vec::new();

        for header in self.inner.headers().get_all(COOKIE).iter() {
            let value = header.to_str().map_err(|_| CookieError::InvalidHeader)?;

            cookies.extend(Cookie::parse(value));
        }

        Ok(cookies)
    }

    /// Procura um cookie pelo nome.
    ///
    /// Retorna Ok(None) quando o cookie não existe.
    /// Retorna erro quando o cookie é inválido, duplicado
    /// ou quando um cabeçalho não pode ser interpretado.
    pub fn cookie(&self, name: &str) -> Result<Option<String>, CookieError> {
        let headers = self
            .inner
            .headers()
            .get_all(COOKIE)
            .iter()
            .map(|header| header.to_str().map_err(|_| CookieError::InvalidHeader))
            .collect::<Result<Vec<_>, _>>()?;

        Cookie::get_unique_from_headers(headers, name)
    }

    /// Devolve o valor de um parâmetro de rota.
    ///
    /// Suporta rotas como `/utilizadores/:id`, permitindo obter o valor do
    /// segmento dinâmico através do nome definido, por exemplo `param("id")`.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Devolve o valor de um parâmetro presente na query string da URL.
    ///
    /// Exemplo: `/produtos?pagina=2&limite=10` pode ser lido com
    /// `query("pagina")`.
    pub fn query(&self, name: &str) -> Option<&str> {
        let query = self.inner.uri().query()?;

        query.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;

            if key == name { Some(value) } else { None }
        })
    }

    /// Cria uma cópia dos metadados para o tratamento de erros.
    pub fn context(&self) -> RequestContext {
        RequestContext {
            method: self.inner.method().clone(),
            path: self.inner.uri().path().to_owned(),
            params: self.params.clone(),
            query: self.inner.uri().query().map(str::to_owned),
            headers: self
                .inner
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    value
                        .to_str()
                        .ok()
                        .map(|value| (name.as_str().to_owned(), value.to_owned()))
                })
                .collect(),
        }
    }

    /// Devolve o corpo HTTP da requisição como bytes.
    ///
    /// O conteúdo é devolvido por referência, sem copiar os dados.
    pub fn body(&self) -> &Bytes {
        self.inner.body()
    }

    /// Devolve o corpo HTTP como texto UTF-8.
    ///
    /// Retorna um erro quando o corpo contém uma sequência de bytes
    /// que não representa texto UTF-8 válido.
    pub fn text(&self) -> Result<&str, std::str::Utf8Error> {
        std::str::from_utf8(self.body())
    }

    /// Converte o corpo HTTP de JSON para uma estrutura Rust.
    ///
    /// O tipo de destino deve implementar `DeserializeOwned`.
    /// Retorna um erro quando o conteúdo não for um JSON válido
    /// ou não corresponder à estrutura esperada.
    pub fn json<T>(&self) -> Result<T, serde_json::Error>
    where
        T: serde::de::DeserializeOwned,
    {
        serde_json::from_slice(self.body())
    }

    /// Converte o corpo HTTP para uma lista de campos de formulário.
    ///
    /// Desc codifica automaticamente caracteres percent-encoded e espaços.
    /// Preserva a ordem original e os campos com nomes repetidos.
    ///
    /// Exemplo: `nome=Fabricio&interesses=Rust&interesses=PHP`.
    pub fn form(&self) -> Vec<(String, String)> {
        if self.is_multipart {
            return self.multipart_fields.clone();
        }

        form_urlencoded::parse(self.body()).into_owned().collect()
    }

    /// Devolve todos os ficheiros recebidos na requisição.
    pub fn files(&self) -> &[UploadedFile] {
        &self.uploaded_files
    }

    /// Procura o primeiro ficheiro associado ao nome de um campo do formulário.
    pub fn file(&self, field_name: &str) -> Option<&UploadedFile> {
        self.uploaded_files
            .iter()
            .find(|file| file.field_name() == field_name)
    }

    /// Associa os dados multipart processados à requisição.
    pub(crate) fn set_multipart_data(
        &mut self,
        fields: Vec<(String, String)>,
        files: Vec<UploadedFile>,
    ) {
        self.multipart_fields = fields;
        self.uploaded_files = files;
        self.is_multipart = true;
    }

    pub(crate) fn set_body(&mut self, body: Bytes) {
        *self.inner.body_mut() = body;
    }

    /// Remove os ficheiros armazenados quando a requisição é rejeitada.
    pub(crate) async fn cleanup_uploaded_files(&mut self) {
        for file in self.uploaded_files.drain(..) {
            let _ = tokio::fs::remove_file(file.path()).await;
        }
    }
}

/// Contexto leve da requisição, disponível durante o tratamento de erros.
#[derive(Debug, Clone)]
pub struct RequestContext {
    method: hyper::http::Method,
    path: String,
    params: Vec<(String, String)>,
    query: Option<String>,
    headers: Vec<(String, String)>,
}

impl RequestContext {
    /// Devolve o método HTTP da requisição.
    pub fn method(&self) -> &hyper::http::Method {
        &self.method
    }

    /// Devolve o caminho da requisição.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Devolve o valor de um parâmetro da rota.
    pub fn param(&self, name: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// Devolve um parâmetro da query string.
    pub fn query(&self, name: &str) -> Option<&str> {
        self.query.as_deref()?.split('&').find_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            (key == name).then_some(value)
        })
    }

    /// Devolve o valor de um cabeçalho HTTP.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            method: hyper::http::Method::GET,
            path: "/teste".to_string(),
            params: Vec::new(),
            query: None,
            headers: Vec::new(),
        }
    }
}
