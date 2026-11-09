#![warn(missing_docs)]

//! # RustPress
//!
//! RustPress é um framework HTTP assíncrono e minimalista para Rust,
//! construído sobre Hyper e Tokio. A API organiza uma aplicação em rotas,
//! middleware, pedidos, respostas, templates e tratamento de erros.
//!
//! ## Exemplo
//!
//! ```no_run
//! use rustpress_web::{App, Request, Response};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
//!     let mut app = App::new();
//!     app.get("/", |_request: Request, response: Response| {
//!         Ok(response.html("<h1>RustPress está a funcionar</h1>"))
//!     })?;
//!     app.listen("127.0.0.1:3000").await?;
//!     Ok(())
//! }
//! ```
//!
//! ## Configuração padrão do Cargo.toml
//!
//! ```toml
//! [package]
//! name = "project_name"
//! version = "0.1.0"
//! edition = "2024"
//!
//! [dependencies]
//! rustpress-web = "0.2.0"
//! tokio = { version = "1", features = ["full"] }
//! ```
//!
//! ## API pública
//!
//! - [`App`]: configuração e arranque do servidor.
//! - [`RouteGroup`]: agrupamento de rotas com prefixos e middleware próprio.
//! - [`Request`] e [`RequestContext`]: metadados e informações dos pedidos.
//! - [`Response`]: construção de respostas HTTP.
//! - [`HandlerError`]: erros devolvidos por handlers.
//! - [`MiddlewareResult`]: controlo da cadeia de middleware avançado.
//! - [`Logger`] e [`Level`]: registo de eventos.
//! - [`TemplateContext`] e [`TemplateError`]: motor de templates.
//! - [`security::Password`]: geração e verificação de hashes de palavras-passe.
//! - [`security::PasswordConfig`]: configuração dos parâmetros do Argon2id.
//! - [`security::PasswordError`]: erros do módulo de segurança.
//! - [`upload::UploadConfig`]: configuração dos limites e armazenamento de uploads.
//! - [`upload::UploadError`]: erros do processamento de uploads.
//! - [`upload::UploadedFile`]: representação de um ficheiro recebido.
//!
//! A API inclui leitura do corpo HTTP, parsing de formulários URL-encoded
//! e `multipart/form-data`, gestão de cookies e sessões, grupos de rotas,
//! ferramentas de segurança para palavras-passe e recepção de ficheiros.
//!
//! ## Upload de ficheiros
//!
//! O processamento de `multipart/form-data` está disponível através do
//! método [`App::use_uploads`]. A funcionalidade deve ser activada
//! explicitamente e permite configurar limites para o tamanho total,
//! tamanho individual dos ficheiros, quantidade de ficheiros e campos.
//!
//! Os ficheiros recebidos são armazenados com nomes gerados pelo framework.
//! Os nomes originais são disponibilizados apenas como metadados.
//!
//! Consulte [`upload::UploadConfig`] e [`upload::UploadedFile`] para
//! conhecer as opções de configuração e os dados disponibilizados.
//!
//! O acesso aos ficheiros recebidos é feito através de
//! [`Request::file`] e [`Request::files`].
//!
//! ## Segurança
//!
//! O módulo [`security`] disponibiliza hashing e verificação de palavras-passe
//! com Argon2id.
//!
//! O módulo [`upload`] aplica limites de tamanho e quantidade para reduzir
//! o risco de consumo excessivo de recursos durante o processamento de uploads.

mod app;
mod handler_error;
pub mod logger;
pub mod middleware;
mod request;
mod response;
mod router;

/// Funcionalidades de segurança, incluindo hashing e verificação de palavras-passe.
pub mod security;
mod server;
pub mod session;
mod template;

/// Funcionalidades de recepção e configuração de ficheiros enviados.
pub mod upload;

pub use app::{App, RouteGroup};
pub use handler_error::HandlerError;
pub use logger::{Level, Logger};
pub use middleware::{AuthMiddleware, CsrfMiddleware, MiddlewareResult, RoleMiddleware};
pub use request::{Request, RequestContext};
pub use response::Response;
pub use template::{TemplateContext, TemplateError};
