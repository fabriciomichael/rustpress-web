//! Abstracções de leitura dos pedidos HTTP.
//!
//! [`Request`] fornece acesso ao método, caminho, cabeçalhos, parâmetros de
//! rota e query string. O corpo HTTP ainda não é exposto pela API pública.

use hyper::{body::Incoming, header::HeaderValue, http::Method};

/// Representa uma requisição HTTP recebida pelo servidor.
///
/// Esta estrutura encapsula a mensagem original do Hyper e acrescenta
/// informações úteis para a aplicação web, nomeadamente os parâmetros de rota
/// extraídos pelo router. A intenção é fornecer uma API simples e idiomática
/// para os handlers registados nas rotas.
pub struct Request {
    inner: hyper::Request<Incoming>,
    params: Vec<(String, String)>,
}

impl Request {
    /// Cria uma nova instância de `Request` com a requisição HTTP original e os
    /// parâmetros de rota já resolvidos.
    pub(crate) fn new(inner: hyper::Request<Incoming>, params: Vec<(String, String)>) -> Self {
        Self { inner, params }
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
}
