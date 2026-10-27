use std::{fmt::Display, fs, path::Path};

use http_body_util::Full;
use hyper::{
    Response as HyperResponse,
    body::Bytes,
    header::{CONTENT_LENGTH, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue},
    http::StatusCode,
};

use crate::TemplateContext;

/// Representa uma resposta HTTP que será enviada ao cliente pelo servidor.
///
/// A estrutura `Response` centraliza a construção de respostas HTTP com status,
/// conteúdo, headers e tipos MIME. Ela permite responder com texto simples,
/// HTML, JSON, templates renderizados e conteúdo binário.
pub struct Response {
    status: StatusCode,
    body: Bytes,
    content_type: HeaderValue,
    headers: HeaderMap,
}

impl Response {
    /// Cria uma resposta padrão com código `200 OK` e tipo de conteúdo em texto
    /// plano.
    pub fn new() -> Self {
        Self {
            status: StatusCode::OK,
            body: Bytes::new(),
            content_type: HeaderValue::from_static("text/plain; charset=utf-8"),
            headers: HeaderMap::new(),
        }
    }

    /// Define o código de estado HTTP da resposta.
    pub fn status(mut self, code: u16) -> Self {
        self.status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        self
    }

    /// Cria uma resposta HTTP de erro com uma mensagem em texto simples.
    pub fn error(code: u16, message: impl Into<String>) -> Self {
        Self::new().status(code).send(message)
    }

    /// Define o corpo da resposta como texto simples.
    pub fn send(mut self, body: impl Into<String>) -> Self {
        self.body = Bytes::from(body.into());
        self
    }

    /// Define a resposta como HTML.
    pub fn html(mut self, body: impl Into<String>) -> Self {
        self.body = Bytes::from(body.into());
        self.content_type = HeaderValue::from_static("text/html; charset=utf-8");
        self
    }

    /// Define o conteúdo da resposta como JSON.
    ///
    /// Esta função ajusta automaticamente o cabeçalho `Content-Type` para
    /// `application/json`.
    pub fn json(mut self, body: impl Into<String>) -> Self {
        self.body = Bytes::from(body.into());

        self.content_type = HeaderValue::from_static("application/json; charset=utf-8");

        self
    }

    /// Carrega um ficheiro do disco e devolve o conteúdo como resposta HTML.
    pub fn render(mut self, path: impl AsRef<Path>) -> Self {
        match fs::read(path) {
            Ok(content) => {
                self.body = Bytes::from(content);
                self.content_type = HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(_) => {
                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = Bytes::from("Não foi possível renderizar a página.");
            }
        }

        self
    }

    /// Renderiza um template substituindo variáveis em formato `{{ nome }}`.
    ///
    /// Esta variante suporta a substituição manual de valores de uma coleção de
    /// pares chave/valor. A função também escapa conteúdo HTML para evitar
    /// injeções simples em templates.
    pub fn render_with<I, K, V>(mut self, path: impl AsRef<Path>, data: I) -> Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: Display,
    {
        let mut template = match fs::read_to_string(path) {
            Ok(content) => content,
            Err(_) => {
                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = Bytes::from("Não foi possível renderizar o template.");
                return self;
            }
        };

        for (key, value) in data {
            let key = key.as_ref();
            let value = escape_html(&value.to_string());

            template = template.replace(&format!("{{{{ {key} }}}}"), &value);

            template = template.replace(&format!("{{{{{key}}}}}"), &value);
        }

        self.body = Bytes::from(template);
        self.content_type = HeaderValue::from_static("text/html; charset=utf-8");

        self
    }

    /// Envia conteúdo binário, por exemplo, uma imagem, um ficheiro PDF ou um
    /// asset estático.
    pub fn send_bytes(mut self, body: impl Into<Bytes>) -> Self {
        self.body = body.into();
        self
    }

    /// Adiciona ou substitui um cabeçalho HTTP na resposta.
    ///
    /// Se o nome do cabeçalho for `Content-Type`, o valor é aplicado também ao
    /// campo interno `content_type` da resposta.
    pub fn header(mut self, name: &str, value: &str) -> Self {
        let Ok(name) = HeaderName::from_bytes(name.as_bytes()) else {
            return self;
        };

        let Ok(value) = HeaderValue::from_str(value) else {
            return self;
        };

        if name == CONTENT_TYPE {
            self.content_type = value;
        } else {
            self.headers.insert(name, value);
        }

        self
    }

    /// Converte a resposta interna para o tipo da biblioteca Hyper antes da
    /// entrega ao cliente.

    pub(crate) fn into_hyper(self) -> HyperResponse<Full<Bytes>> {
        let body_length = self.body.len();

        let mut response = HyperResponse::new(Full::new(self.body));

        *response.status_mut() = self.status;

        response
            .headers_mut()
            .insert(CONTENT_TYPE, self.content_type);

        response.headers_mut().extend(self.headers);

        if !response.headers().contains_key(CONTENT_LENGTH) {
            if let Ok(value) = HeaderValue::from_str(&body_length.to_string()) {
                response.headers_mut().insert(CONTENT_LENGTH, value);
            }
        }

        response
    }

    /// Renderiza um template HTML usando um contexto estruturado de dados.
    ///
    /// Este método lê um ficheiro de template e aplica os valores armazenados em
    /// `TemplateContext`, permitindo uma integração mais robusta com páginas
    /// dinâmicas.
    pub fn render_template(mut self, path: &str, context: &TemplateContext) -> Self {
        match std::fs::read_to_string(path) {
            Ok(template) => {
                let html = context.render(&template);

                self.body = bytes::Bytes::from(html);
                self.content_type = http::HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(error) => {
                eprintln!("Erro ao carregar o template '{}': {}", path, error);

                self.status = http::StatusCode::INTERNAL_SERVER_ERROR;

                self.body = bytes::Bytes::from("Erro interno ao renderizar a página.");

                self.content_type = http::HeaderValue::from_static("text/plain; charset=utf-8");
            }
        }

        self
    }

    pub fn render_view(
        mut self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
        context: &crate::TemplateContext,
    ) -> Self {
        match context.render_with_layout(page_path, layout_path, partials_dir) {
            Ok(html) => {
                self.body = bytes::Bytes::from(html);
                self.content_type = http::HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(error) => {
                eprintln!("Erro ao renderizar a página: {error}");

                self.status = http::StatusCode::INTERNAL_SERVER_ERROR;
                self.body = bytes::Bytes::from("Erro interno ao renderizar a página.");
                self.content_type = http::HeaderValue::from_static("text/plain; charset=utf-8");
            }
        }

        self
    }

    /// Renderiza uma página de erro personalizada usando um layout e partials.
    ///
    /// Se a renderização falhar, mantém o código HTTP original e devolve
    /// uma mensagem simples como alternativa.
    pub fn error_page(
        code: u16,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
        context: &TemplateContext,
    ) -> Self {
        let status = StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        match context.render_with_layout(page_path, layout_path, partials_dir) {
            Ok(html) => Self::new().status(status.as_u16()).html(html),
            Err(error) => {
                eprintln!("Erro ao renderizar a página de erro {}: {}", code, error);

                Self::error(status.as_u16(), format!("Erro HTTP {}", status.as_u16()))
            }
        }
    }
}

/// Escapa sequências HTML para impedir que valores dinâmicos quebrem a
/// estrutura do template no browser.
fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

impl Default for Response {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_error_response_with_status_and_message() {
        let response = Response::error(404, "Not Found");

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(&response.body[..], b"Not Found");
    }

    #[test]
    fn creates_method_not_allowed_response() {
        let response = Response::error(405, "Method Not Allowed");

        assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(&response.body[..], b"Method Not Allowed");
    }

    #[test]
    fn error_page_preserves_status_when_template_is_missing() {
        let context = TemplateContext::new();

        let response = Response::error_page(
            404,
            "views/errors/ficheiro_inexistente.html",
            "views/layouts/layout_inexistente.html",
            "views/partials",
            &context,
        );

        assert_eq!(response.status, StatusCode::NOT_FOUND);

        let body =
            String::from_utf8(response.body.to_vec()).expect("O corpo da resposta deve ser UTF-8");

        assert_eq!(body, "Erro HTTP 404");
    }

    #[test]
    fn error_page_preserves_405_and_500_when_templates_are_missing() {
        let context = TemplateContext::new();

        for code in [405, 500] {
            let response = Response::error_page(
                code,
                "views/errors/ficheiro_inexistente.html",
                "views/layouts/layout_inexistente.html",
                "views/partials",
                &context,
            );

            assert_eq!(
                response.status.as_u16(),
                code,
                "O estado HTTP deve ser preservado: {code}"
            );

            let body = String::from_utf8(response.body.to_vec())
                .expect("O corpo da resposta deve ser UTF-8");

            assert_eq!(body, format!("Erro HTTP {code}"));
        }
    }
}
