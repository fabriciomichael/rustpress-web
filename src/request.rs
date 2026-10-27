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
}
