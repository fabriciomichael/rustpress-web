#![warn(missing_docs)]

//! # RustPress
//!
//! RustPress é um framework HTTP assíncrono e minimalista para Rust, construído sobre Hyper e Tokio.
//! A API organiza uma aplicação em rotas, middleware, pedidos, respostas, templates e tratamento de erros.
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
//!
//! A API inclui leitura do corpo HTTP, parsing de formulários URL-encoded,
//! gestão de cookies e sessões, grupos de rotas e ferramentas de segurança
//! para palavras-passe.
//!
//! O processamento de `multipart/form-data` e o upload de ficheiros
//! ainda não estão disponíveis.

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

pub use app::{App, RouteGroup};
pub use handler_error::HandlerError;
pub use logger::{Level, Logger};
pub use middleware::{AuthMiddleware, CsrfMiddleware, MiddlewareResult, RoleMiddleware};
pub use request::{Request, RequestContext};
pub use response::Response;
pub use template::{TemplateContext, TemplateError};
