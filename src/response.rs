//! Construção de respostas HTTP.
//!
//! [`Response`] mantém o estado, os cabeçalhos e o corpo até ser convertido
//! para uma resposta Hyper. Os métodos de construção consomem e devolvem
//! `Self`, permitindo encadear chamadas.
//!
//! # Exemplo
//!
//! ```
//! use rustpress::Response;
//! let response = Response::new().status(201).header("X-Application", "RustPress").json(r#"{"created":true}"#);
//! ```

use std::{fs, path::Path};

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
    content_length: Option<u64>,
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
            content_length: None,
        }
    }

    ///Sets the HTTP status code of the response.
    pub fn status(mut self, code: u16) -> Self {
        self.status = if (100..=599).contains(&code) {
            StatusCode::from_u16(code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };

        self
    }

    /// Cria uma resposta HTTP de erro com uma mensagem em texto simples.
    pub fn error(code: u16, message: impl Into<String>) -> Self {
        Self::new().status(code).send(message)
    }

    /// Cria uma resposta HTTP de erro no formato JSON.
    pub fn json_error(code: u16, message: impl Into<String>) -> Self {
        let status = if (400..=599).contains(&code) {
            code
        } else {
            500
        };

        let body = serde_json::json!({
            "error": {
                "status": status,
                "message": message.into()
            }
        });

        Self::new().status(status).json(body.to_string())
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

    /// Loads a file from disk and returns its contents as an HTML response.
    pub fn render_file(mut self, path: impl AsRef<Path>) -> Self {
        match fs::read(path) {
            Ok(content) => {
                self.body = Bytes::from(content);
                self.content_type = HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(error) => {
                eprintln!("Error loading HTML file: {error}");

                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = Bytes::from("Internal error while loading the HTML file.");
                self.content_type = HeaderValue::from_static("text/plain; charset=utf-8");
            }
        }

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
        let body_length = self.content_length.unwrap_or(self.body.len() as u64);

        let mut response = HyperResponse::new(Full::new(self.body));

        *response.status_mut() = self.status;

        response
            .headers_mut()
            .insert(CONTENT_TYPE, self.content_type);

        response.headers_mut().extend(self.headers);

        if let Ok(value) = HeaderValue::from_str(&body_length.to_string()) {
            response.headers_mut().insert(CONTENT_LENGTH, value);
        }

        response
    }

    /// Renders an HTML template using the provided context.
    pub fn render_template(mut self, path: &str, context: &TemplateContext) -> Self {
        match std::fs::read_to_string(path) {
            Ok(template) => {
                let html = context.render(&template);

                self.body = Bytes::from(html);
                self.content_type = HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(error) => {
                eprintln!("Error loading template '{path}': {error}");

                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = Bytes::from("Internal error while rendering the page.");
                self.content_type = HeaderValue::from_static("text/plain; charset=utf-8");
            }
        }

        self
    }

    /// Renderiza uma página com layout e validação rigorosa das variáveis.
    ///
    /// Devolve HTTP 500 quando uma variável não está definida ou ocorre
    /// um erro ao carregar os templates ou os partials.
    pub fn render(
        mut self,
        page_path: &str,
        layout_path: &str,
        partials_dir: &str,
        context: &TemplateContext,
    ) -> Self {
        match context.render_with_layout_strict(page_path, layout_path, partials_dir) {
            Ok(html) => {
                self.body = Bytes::from(html);
                self.content_type = HeaderValue::from_static("text/html; charset=utf-8");
            }
            Err(error) => {
                eprintln!("Error rendering the page: {error}");

                self.status = StatusCode::INTERNAL_SERVER_ERROR;
                self.body = Bytes::from("Internal error while rendering the page.");
                self.content_type = HeaderValue::from_static("text/plain; charset=utf-8");
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

    pub(crate) fn content_length(mut self, length: u64) -> Self {
        self.content_length = Some(length);
        self
    }
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
    fn status_accepts_valid_http_code() {
        let response = Response::new().status(201);

        assert_eq!(response.status, StatusCode::CREATED);
    }

    #[test]
    fn status_falls_back_to_500_for_invalid_http_code() {
        let response = Response::new().status(999);

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn header_sets_custom_header() {
        let response = Response::new().header("X-Request-Id", "abc123");

        assert_eq!(response.headers.get("X-Request-Id").unwrap(), "abc123");
    }

    #[test]
    fn header_updates_content_type() {
        let response = Response::new().header("Content-Type", "application/xml");

        assert_eq!(
            response.content_type,
            HeaderValue::from_static("application/xml")
        );
    }

    #[test]
    fn send_returns_plain_text_and_preserves_body() {
        let expected_body = "Resposta do RustPress";

        let response = Response::new().send(expected_body);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/plain; charset=utf-8")
        );
        assert_eq!(&response.body[..], expected_body.as_bytes());
    }

    #[test]
    fn html_returns_html_content_type_and_preserves_body() {
        let expected_html = "<h1>RustPress</h1>";

        let response = Response::new().html(expected_html);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/html; charset=utf-8")
        );
        assert_eq!(&response.body[..], expected_html.as_bytes());
    }

    #[test]
    fn creates_method_not_allowed_response() {
        let response = Response::error(405, "Method Not Allowed");

        assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(&response.body[..], b"Method Not Allowed");
    }

    #[test]
    fn json_returns_json_content_type_and_preserves_body() {
        let expected_json = r#"{"message":"RustPress","success":true}"#;

        let response = Response::new().json(expected_json);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("application/json; charset=utf-8")
        );
        assert_eq!(&response.body[..], expected_json.as_bytes());
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
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/plain; charset=utf-8")
        );
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

    #[test]
    fn render_returns_500_for_undefined_variable() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-strict-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        fs::write(&page, "<h1>{{usuario}}</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let context = TemplateContext::new();

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_returns_500_for_undefined_variable_with_strict_validation() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-strict-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        fs::write(&page, "<h1>{{usuario}}</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let context = TemplateContext::new();

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_returns_html_for_valid_template() {
        use serde_json::json;
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-valid-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        fs::write(&page, "<h1>{{titulo}}</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let mut context = TemplateContext::new();
        context.set("titulo", json!("RustPress"));

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/html; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert!(body.contains("<h1>RustPress</h1>"));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_returns_html_for_valid_template_with_strict_validation() {
        use serde_json::json;
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-valid-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        fs::write(&page, "<h1>{{titulo}}</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let mut context = TemplateContext::new();
        context.set("titulo", json!("RustPress"));

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/html; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert!(body.contains("<h1>RustPress</h1>"));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_returns_500_for_invalid_syntax_with_strict_validation() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-invalid-syntax-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        // O bloco if está aberto e não tem fecho.
        fs::write(&page, "<h1>{{#if titulo}}Olá</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let context = TemplateContext::new();

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);

        let body = String::from_utf8(response.body.to_vec()).unwrap();

        assert_eq!(body, "Internal error while rendering the page.");
        assert!(!body.contains("Erro de sintaxe"));
        assert!(!body.contains("TemplateError"));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_returns_500_for_invalid_syntax() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let base = std::env::temp_dir().join(format!("rustpress-response-invalid-syntax-{unique}"));

        let partials = base.join("partials");
        fs::create_dir_all(&partials).unwrap();

        let page = base.join("pagina.html");
        let layout = base.join("layout.html");

        // O bloco if está aberto e não tem fecho.
        fs::write(&page, "<h1>{{#if titulo}}Olá</h1>").unwrap();
        fs::write(&layout, "<html><body>{{{conteudo}}}</body></html>").unwrap();

        let context = TemplateContext::new();

        let response = Response::new().render(
            page.to_str().unwrap(),
            layout.to_str().unwrap(),
            partials.to_str().unwrap(),
            &context,
        );

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);

        let body = String::from_utf8(response.body.to_vec()).unwrap();

        assert_eq!(body, "Internal error while rendering the page.");
        assert!(!body.contains("Erro de sintaxe"));
        assert!(!body.contains("TemplateError"));

        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn render_template_returns_html_for_valid_template() {
        use serde_json::json;
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!("rustpress-render-template-{unique}.html"));

        fs::write(&path, "<h1>{{titulo}}</h1>").unwrap();

        let mut context = TemplateContext::new();
        context.set("titulo", json!("RustPress"));

        let response = Response::new().render_template(path.to_str().unwrap(), &context);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/html; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert!(body.contains("<h1>RustPress</h1>"));

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn render_template_returns_500_when_template_is_missing() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!("rustpress-missing-template-{unique}.html"));

        let response =
            Response::new().render_template(path.to_str().unwrap(), &TemplateContext::new());

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/plain; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert_eq!(body, "Internal error while rendering the page.");

        // Garante que o ficheiro de teste não ficou criado.
        assert!(!path.exists());
    }

    #[test]
    fn render_file_returns_html_for_existing_file() {
        use std::{
            fs,
            time::{SystemTime, UNIX_EPOCH},
        };

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!("rustpress-render-file-{unique}.html"));

        let expected_html = "<h1>RustPress</h1>";
        fs::write(&path, expected_html).unwrap();

        let response = Response::new().render_file(&path);

        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/html; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert_eq!(body, expected_html);

        fs::remove_file(&path).unwrap();
    }

    #[test]
    fn render_file_returns_500_when_file_is_missing() {
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let path = std::env::temp_dir().join(format!("rustpress-missing-file-{unique}.html"));

        let response = Response::new().render_file(&path);

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("text/plain; charset=utf-8")
        );

        let body = String::from_utf8(response.body.to_vec()).unwrap();
        assert_eq!(body, "Internal error while loading the HTML file.");

        assert!(!path.exists());
    }

    #[test]
    fn header_ignores_invalid_header_name() {
        let response = Response::new().header("Invalid Header", "value");

        assert!(response.headers.is_empty());
    }

    #[test]
    fn header_ignores_invalid_header_value() {
        let response = Response::new().header("X-Test", "value\ninvalid");

        assert!(response.headers.is_empty());
    }

    #[test]
    fn into_hyper_preserves_status_body_and_headers() {
        let response = Response::new()
            .status(201)
            .header("X-Request-Id", "abc123")
            .send("Criado com sucesso");

        let hyper_response = response.into_hyper();

        assert_eq!(hyper_response.status(), StatusCode::CREATED);

        assert_eq!(
            hyper_response.headers().get("X-Request-Id").unwrap(),
            "abc123"
        );

        assert_eq!(
            hyper_response.headers().get(CONTENT_TYPE).unwrap(),
            "text/plain; charset=utf-8"
        );

        assert_eq!(hyper_response.headers().get(CONTENT_LENGTH).unwrap(), "18");
    }

    #[tokio::test]
    async fn into_hyper_preserves_body_content() {
        use http_body_util::BodyExt;

        let expected_body = "Criado com sucesso";

        let response = Response::new().send(expected_body);
        let hyper_response = response.into_hyper();

        let body = hyper_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();

        assert_eq!(&body[..], expected_body.as_bytes());
    }

    #[test]
    fn into_hyper_corrects_inconsistent_content_length() {
        let response = Response::new()
            .header("Content-Length", "100")
            .send("Teste");

        let hyper_response = response.into_hyper();

        assert_eq!(hyper_response.headers().get(CONTENT_LENGTH).unwrap(), "5");
    }

    #[test]
    fn json_error_returns_status_message_and_json_content_type() {
        let response = Response::json_error(404, "Rota não encontrada");

        assert_eq!(response.status, StatusCode::NOT_FOUND);
        assert_eq!(
            response.content_type,
            HeaderValue::from_static("application/json; charset=utf-8")
        );

        let body: serde_json::Value =
            serde_json::from_slice(&response.body).expect("O corpo deve ser JSON válido");

        assert_eq!(body["error"]["status"], 404);
        assert_eq!(body["error"]["message"], "Rota não encontrada");
    }

    #[test]
    fn json_error_escapes_special_characters() {
        let message = "Erro com \"aspas\" e \\ barra";
        let response = Response::json_error(400, message);

        let body: serde_json::Value =
            serde_json::from_slice(&response.body).expect("O corpo deve ser JSON válido");

        assert_eq!(body["error"]["message"], message);
    }

    #[test]
    fn json_error_uses_500_for_non_error_status_codes() {
        let response = Response::json_error(200, "Erro interno");

        assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);

        let body: serde_json::Value =
            serde_json::from_slice(&response.body).expect("O corpo deve ser JSON válido");

        assert_eq!(body["error"]["status"], 500);
    }
}
