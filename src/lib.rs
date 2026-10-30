#![warn(missing_docs)]

//! # RustPress
//!
//! RustPress é um framework HTTP assíncrono e minimalista para Rust, construído sobre Hyper e Tokio.
//! A API organiza uma aplicação em rotas, middleware, pedidos, respostas, templates e tratamento de erros.
//!
//! ## Exemplo
//!
//! ```no_run
//! use rustpress::{App, Request, Response};
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
//! ## API pública
//!
//! - [`App`]: configuração e arranque do servidor.
//! - [`Request`] e [`RequestContext`]: metadados dos pedidos.
//! - [`Response`]: construção de respostas HTTP.
//! - [`HandlerError`]: erros devolvidos por handlers.
//! - [`MiddlewareResult`]: controlo da cadeia de middleware.
//! - [`Logger`] e [`Level`]: registo de eventos.
//! - [`TemplateContext`] e [`TemplateError`]: motor de templates.
//!
//! A leitura do corpo HTTP, parsing de formulários e uploads multipart ainda não fazem parte da API pública.

mod app;
mod handler_error;
pub mod logger;
pub mod middleware;
mod request;
mod response;
mod router;
mod server;
mod template;

pub use app::App;
pub use handler_error::HandlerError;
pub use logger::{Level, Logger};
pub use middleware::MiddlewareResult;
pub use request::{Request, RequestContext};
pub use response::Response;
pub use template::{TemplateContext, TemplateError};
