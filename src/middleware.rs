//! Middleware types and control tools.

use crate::{HandlerError, Request, Response};

/// Result returned by an advanced middleware.
pub enum MiddlewareResult {
    /// Continues execution of the middleware chain.
    Next,

    /// Stops the chain and returns an HTTP response.
    Respond(Response),

    /// Stops the chain and forwards the error to central handling.
    Error(HandlerError),
}

/// Configurable authentication middleware.
pub struct AuthMiddleware<F> {
    check: F,
    unauthorized_response: Box<dyn Fn() -> Response + Send + Sync>,
}

impl<F> AuthMiddleware<F>
where
    F: Fn(&Request) -> Result<bool, HandlerError> + Send + Sync + 'static,
{
    /// Creates a middleware with an authentication check function.
    pub fn new(check: F) -> Self {
        Self {
            check,
            unauthorized_response: Box::new(|| {
                Response::error(401, "Não estás autenticado para aceder a este recurso.")
            }),
        }
    }

    /// Sets the HTTP response returned when the user is not authenticated.
    ///
    /// The provided function is executed whenever a response is needed.
    pub fn unauthorized_response<G>(mut self, response: G) -> Self
    where
        G: Fn() -> Response + Send + Sync + 'static,
    {
        self.unauthorized_response = Box::new(response);
        self
    }

    /// Checks authentication and determines the middleware result.
    ///
    /// Returns `Next` when authenticated, `Respond` when unauthenticated,
    /// and `Error` when an authentication check fails.
    pub fn handle(&self, request: &Request) -> MiddlewareResult {
        match (self.check)(request) {
            Ok(true) => MiddlewareResult::Next,
            Ok(false) => MiddlewareResult::Respond((self.unauthorized_response)()),
            Err(error) => MiddlewareResult::Error(error),
        }
    }
}

/// Role-based authorization middleware (RBAC).
pub struct RoleMiddleware {
    allowed_roles: Vec<String>,
    unauthorized_response: Box<dyn Fn() -> Response + Send + Sync>,
    forbidden_response: Box<dyn Fn() -> Response + Send + Sync>,
}

impl RoleMiddleware {
    /// Creates a middleware that allows a specific role.
    pub fn new(role: impl Into<String>) -> Self {
        Self::from_roles(vec![role.into()])
    }

    /// Creates a middleware that allows any of the specified roles.
    pub fn any_of<I, S>(roles: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::from_roles(roles.into_iter().map(Into::into).collect())
    }

    fn from_roles(allowed_roles: Vec<String>) -> Self {
        Self {
            allowed_roles,
            unauthorized_response: Box::new(|| {
                Response::error(401, "Não estás autenticado para aceder a este recurso.")
            }),
            forbidden_response: Box::new(|| {
                Response::error(403, "Não tens permissão para aceder a este recurso.")
            }),
        }
    }

    /// Customizes the response returned when the user is not authenticated.
    pub fn unauthorized_response<G>(mut self, response: G) -> Self
    where
        G: Fn() -> Response + Send + Sync + 'static,
    {
        self.unauthorized_response = Box::new(response);
        self
    }

    /// Customizes the response returned when the user is not authorized.
    pub fn forbidden_response<G>(mut self, response: G) -> Self
    where
        G: Fn() -> Response + Send + Sync + 'static,
    {
        self.forbidden_response = Box::new(response);
        self
    }

    /// Checks the user's identity and roles.
    pub fn handle(&self, request: &Request) -> MiddlewareResult {
        let Some(session) = request.session() else {
            return MiddlewareResult::Respond((self.unauthorized_response)());
        };

        let session = match session.lock() {
            Ok(session) => session,
            Err(_) => {
                return MiddlewareResult::Error(HandlerError::with_details(
                    "Erro interno do servidor.",
                    "Não foi possível bloquear a sessão do utilizador.",
                ));
            }
        };

        let Some(_user_id) = session.get::<u64>("user_id").filter(|id| *id > 0) else {
            return MiddlewareResult::Respond((self.unauthorized_response)());
        };

        let Some(role) = session.get::<String>("role") else {
            return MiddlewareResult::Respond((self.forbidden_response)());
        };

        if self.allowed_roles.iter().any(|allowed| allowed == &role) {
            MiddlewareResult::Next
        } else {
            MiddlewareResult::Respond((self.forbidden_response)())
        }
    }
}

/// Key used to store the CSRF token in the session.
const CSRF_SESSION_KEY: &str = "_rustpress_csrf_token";

/// CSRF attack protection middleware.
pub struct CsrfMiddleware {
    forbidden_response: Box<dyn Fn() -> Response + Send + Sync>,
}

impl CsrfMiddleware {
    /// Creates a CSRF protection middleware.
    pub fn new() -> Self {
        Self {
            forbidden_response: Box::new(|| {
                Response::error(403, "Token CSRF ausente ou inválido.")
            }),
        }
    }

    /// Customizes the response returned when the token is invalid.
    pub fn forbidden_response<F>(mut self, response: F) -> Self
    where
        F: Fn() -> Response + Send + Sync + 'static,
    {
        self.forbidden_response = Box::new(response);
        self
    }

    /// Gets the CSRF token associated with the request.
    pub fn token(&self, request: &Request) -> Result<Option<String>, HandlerError> {
        let Some(session) = request.session() else {
            return Ok(None);
        };

        let session = session.lock().map_err(|_| {
            HandlerError::with_details(
                "Erro interno do servidor.",
                "Não foi possível bloquear a sessão CSRF.",
            )
        })?;

        Ok(session.get::<String>(CSRF_SESSION_KEY))
    }

    /// Checks and validates the request's CSRF token.
    pub fn handle(&self, request: &Request) -> MiddlewareResult {
        let method = request.method().as_str();

        // Safe HTTP methods do not require validation.
        if matches!(method, "GET" | "HEAD" | "OPTIONS" | "TRACE") {
            let Some(session) = request.session() else {
                return MiddlewareResult::Next;
            };

            let mut session = match session.lock() {
                Ok(session) => session,
                Err(_) => {
                    return MiddlewareResult::Error(HandlerError::with_details(
                        "Erro interno do servidor.",
                        "Não foi possível bloquear a sessão CSRF.",
                    ));
                }
            };

            // Generate a token if it does not exist yet.
            if !session.contains(CSRF_SESSION_KEY) {
                let token = format!(
                    "{}{}",
                    uuid::Uuid::new_v4().simple(),
                    uuid::Uuid::new_v4().simple()
                );

                if let Err(error) = session.insert(CSRF_SESSION_KEY, token) {
                    return MiddlewareResult::Error(HandlerError::with_details(
                        "Erro interno do servidor.",
                        error.to_string(),
                    ));
                }
            }

            return MiddlewareResult::Next;
        }

        let Some(session) = request.session() else {
            return MiddlewareResult::Respond((self.forbidden_response)());
        };

        let expected = match session.lock() {
            Ok(session) => session.get::<String>(CSRF_SESSION_KEY),
            Err(_) => {
                return MiddlewareResult::Error(HandlerError::with_details(
                    "Erro interno do servidor.",
                    "Não foi possível bloquear a sessão CSRF.",
                ));
            }
        };

        let Some(expected) = expected else {
            return MiddlewareResult::Respond((self.forbidden_response)());
        };

        // First, look for the token in the HTTP header.
        let header_token = request
            .header("X-CSRF-Token")
            .and_then(|value| value.to_str().ok());

        // If the header is absent, look for the field in the form.
        let form_token = if header_token.is_none() {
            request
                .form()
                .into_iter()
                .find(|(key, _)| key == "_csrf")
                .map(|(_, value)| value)
        } else {
            None
        };

        let supplied = header_token.or(form_token.as_deref());

        if supplied.is_some_and(|token| constant_time_eq(token, &expected)) {
            MiddlewareResult::Next
        } else {
            MiddlewareResult::Respond((self.forbidden_response)())
        }
    }
}

impl Default for CsrfMiddleware {
    fn default() -> Self {
        Self::new()
    }
}

/// Compares two tokens without ending the comparison at the first different byte.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }

    a.as_bytes()
        .iter()
        .zip(b.as_bytes())
        .fold(0_u8, |diff, (x, y)| diff | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::{AuthMiddleware, CsrfMiddleware, MiddlewareResult};
    use crate::{HandlerError, Request, Response};
    use bytes::Bytes;
    use http::HeaderValue;
    use hyper::{Method, Request as HyperRequest};

    fn create_request() -> Request {
        let inner = HyperRequest::builder()
            .method(Method::GET)
            .uri("/dashboard")
            .body(Bytes::new())
            .unwrap();

        Request::new(inner, Vec::new())
    }

    fn create_csrf_request(method: Method, body: Bytes, token: Option<&str>) -> Request {
        let mut builder = HyperRequest::builder().method(method).uri("/dashboard");

        if let Some(token) = token {
            builder = builder.header("X-CSRF-Token", HeaderValue::from_str(token).unwrap());
        }

        let inner = builder.body(body).unwrap();

        Request::new(inner, Vec::new())
    }

    fn create_csrf_session_request(method: Method, body: Bytes) -> Request {
        let mut request = create_csrf_request(method, body, None);
        let session = Session::new("test-session".to_string());

        request.set_session(Arc::new(Mutex::new(session)));

        request
    }

    #[test]
    fn authenticated_user_continues_middleware_chain() {
        let auth = AuthMiddleware::new(|_| Ok(true));
        let request = create_request();

        assert!(matches!(auth.handle(&request), MiddlewareResult::Next));
    }

    #[test]
    fn unauthenticated_user_receives_401() {
        let auth = AuthMiddleware::new(|_| Ok(false));
        let request = create_request();

        let MiddlewareResult::Respond(response) = auth.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        let response = response.into_hyper();

        assert_eq!(response.status(), 401);
    }

    #[test]
    fn unauthenticated_user_can_be_redirected() {
        let auth = AuthMiddleware::new(|_| Ok(false))
            .unauthorized_response(|| Response::redirect("/login"));

        let request = create_request();

        let MiddlewareResult::Respond(response) = auth.handle(&request) else {
            panic!("Era esperada uma resposta de redireccionamento.");
        };

        let response = response.into_hyper();

        assert_eq!(response.status(), 302);
        assert_eq!(response.headers().get("Location").unwrap(), "/login");
    }

    #[test]
    fn authentication_error_is_forwarded() {
        let auth = AuthMiddleware::new(|_| {
            Err(HandlerError::with_details(
                "Erro na autenticação.",
                "Falha ao consultar os dados do utilizador.",
            ))
        });

        let request = create_request();

        let MiddlewareResult::Error(error) = auth.handle(&request) else {
            panic!("Era esperado um erro de autenticação.");
        };

        assert_eq!(error.status(), 500);
        assert_eq!(error.public_message(), "Erro na autenticação.");
        assert_eq!(
            error.details(),
            Some("Falha ao consultar os dados do utilizador.")
        );
    }

    use crate::session::Session;
    use std::sync::{Arc, Mutex};

    fn create_authenticated_request(user_id: u64, role: &str) -> Request {
        let mut request = create_request();
        let mut session = Session::new("test-session".to_string());

        session.insert("user_id", user_id).unwrap();
        session.insert("role", role).unwrap();

        request.set_session(Arc::new(Mutex::new(session)));
        request
    }

    #[test]
    fn authorized_role_continues_middleware_chain() {
        let middleware = super::RoleMiddleware::new("admin");
        let request = create_authenticated_request(1, "admin");

        assert!(matches!(
            middleware.handle(&request),
            MiddlewareResult::Next
        ));
    }

    #[test]
    fn unauthorized_role_receives_403() {
        let middleware = super::RoleMiddleware::new("admin");
        let request = create_authenticated_request(1, "professor");

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 403);
    }

    #[test]
    fn missing_session_receives_401() {
        let middleware = super::RoleMiddleware::new("admin");
        let request = create_request();

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 401);
    }

    #[test]
    fn missing_user_id_receives_401() {
        let middleware = super::RoleMiddleware::new("admin");
        let mut request = create_request();
        let mut session = Session::new("test-session".to_string());

        session.insert("role", "admin").unwrap();
        request.set_session(Arc::new(Mutex::new(session)));

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 401);
    }

    #[test]
    fn zero_user_id_receives_401() {
        let middleware = super::RoleMiddleware::new("admin");
        let request = create_authenticated_request(0, "admin");

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 401);
    }

    #[test]
    fn missing_role_receives_403() {
        let middleware = super::RoleMiddleware::new("admin");
        let mut request = create_request();
        let mut session = Session::new("test-session".to_string());

        session.insert("user_id", 1_u64).unwrap();
        request.set_session(Arc::new(Mutex::new(session)));

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 403);
    }

    #[test]
    fn any_of_accepts_multiple_roles() {
        let middleware = super::RoleMiddleware::any_of(["admin", "professor"]);
        let request = create_authenticated_request(1, "professor");

        assert!(matches!(
            middleware.handle(&request),
            MiddlewareResult::Next
        ));
    }

    #[test]
    fn csrf_generates_token_for_safe_request() {
        let middleware = CsrfMiddleware::new();
        let request = create_csrf_session_request(Method::GET, Bytes::new());

        assert!(matches!(
            middleware.handle(&request),
            MiddlewareResult::Next
        ));

        let token = middleware.token(&request).unwrap();
        assert!(token.is_some());
        assert!(!token.unwrap().is_empty());
    }

    #[test]
    fn csrf_token_remains_the_same_for_same_session() {
        let middleware = CsrfMiddleware::new();
        let request = create_csrf_session_request(Method::GET, Bytes::new());

        middleware.handle(&request);
        let first = middleware.token(&request).unwrap();

        middleware.handle(&request);
        let second = middleware.token(&request).unwrap();

        assert_eq!(first, second);
    }

    #[test]
    fn csrf_rejects_unsafe_request_without_session() {
        let middleware = CsrfMiddleware::new();
        let request = create_csrf_request(Method::POST, Bytes::new(), None);

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 403);
    }

    #[test]
    fn csrf_rejects_unsafe_request_without_token() {
        let middleware = CsrfMiddleware::new();
        let request = create_csrf_session_request(Method::GET, Bytes::new());

        middleware.handle(&request);

        let mut post = create_csrf_session_request(Method::POST, Bytes::new());
        post.set_session(request.session().unwrap());

        let MiddlewareResult::Respond(response) = middleware.handle(&post) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 403);
    }

    #[test]
    fn csrf_rejects_invalid_header_token() {
        let middleware = CsrfMiddleware::new();
        let get = create_csrf_session_request(Method::GET, Bytes::new());

        middleware.handle(&get);

        let mut post = create_csrf_request(Method::POST, Bytes::new(), Some("token-invalido"));
        post.set_session(get.session().unwrap());

        let MiddlewareResult::Respond(response) = middleware.handle(&post) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 403);
    }

    #[test]
    fn csrf_accepts_valid_form_token() {
        let middleware = CsrfMiddleware::new();
        let get = create_csrf_session_request(Method::GET, Bytes::new());

        middleware.handle(&get);

        let token = middleware.token(&get).unwrap().unwrap();
        let body = Bytes::from(format!("_csrf={}", token));

        let mut post = create_csrf_session_request(Method::POST, body);
        post.set_session(get.session().unwrap());

        assert!(matches!(middleware.handle(&post), MiddlewareResult::Next));
    }

    #[test]
    fn csrf_safe_methods_do_not_require_token_and_generate_session_token() {
        let middleware = CsrfMiddleware::new();

        for method in [Method::GET, Method::HEAD, Method::OPTIONS, Method::TRACE] {
            let request = create_csrf_session_request(method.clone(), Bytes::new());

            assert!(matches!(
                middleware.handle(&request),
                MiddlewareResult::Next
            ));

            let token = middleware.token(&request).unwrap();

            assert!(
                token.is_some(),
                "O método {} deveria gerar um token CSRF.",
                method
            );
        }
    }

    #[test]
    fn csrf_custom_forbidden_response_is_used() {
        let middleware =
            CsrfMiddleware::new().forbidden_response(|| Response::error(419, "CSRF inválido."));

        let request = create_csrf_session_request(Method::POST, Bytes::new());

        let MiddlewareResult::Respond(response) = middleware.handle(&request) else {
            panic!("Era esperada uma resposta HTTP.");
        };

        assert_eq!(response.into_hyper().status(), 419);
    }

    #[test]
    fn csrf_accepts_valid_header_token() {
        let middleware = CsrfMiddleware::new();
        let get = create_csrf_session_request(Method::GET, Bytes::new());

        middleware.handle(&get);

        let token = middleware.token(&get).unwrap().unwrap();

        let mut post = create_csrf_request(Method::POST, Bytes::new(), Some(&token));

        post.set_session(get.session().unwrap());

        assert!(matches!(middleware.handle(&post), MiddlewareResult::Next));
    }
}
