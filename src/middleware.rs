//! Tipos de controlo para middleware.
//!
//! [`MiddlewareResult`] permite que um middleware avançado continue a cadeia,
//! interrompa o pedido com uma resposta ou encaminhe um erro para o handler
//! central da aplicação.

use crate::{HandlerError, Response};

/// Resultado devolvido por um middleware avançado.
pub enum MiddlewareResult {
    /// Continua a execução da cadeia de middlewares.
    Next,

    /// Interrompe a cadeia e devolve uma resposta HTTP.
    Respond(Response),

    /// Interrompe a cadeia e encaminha o erro para o tratamento central.
    Error(HandlerError),
}
