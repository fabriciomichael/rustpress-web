//! Correspondência de rotas e despacho de pedidos.
//!
//! O router é um componente interno: armazena handlers por método e padrão
//! de caminho, extrai parâmetros dinâmicos, executa middlewares e encaminha
//! erros para o handler global configurado em [`crate::App`].
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::Duration,
};

use bytes::{Bytes, BytesMut};
use http_body_util::BodyExt;
use http_body_util::Full;
use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming, http::Method};

use crate::{
    HandlerError, MiddlewareResult, TemplateContext,
    logger::Logger,
    request::Request,
    response::Response,
    session::{Cookie, MemoryStore, SessionConfig, SessionManager},
};

const MAX_BODY_SIZE: usize = 1024 * 1024;

/// Tipo que representa um handler de rota.
///
/// Um handler recebe a requisição processada pelo framework e a resposta em
/// construção e devolve a resposta final a enviar ao cliente.
pub(crate) type Handler =
    Arc<dyn Fn(Request, Response) -> Result<Response, HandlerError> + Send + Sync>;

#[derive(Clone)]
struct RouteEntry {
    handler: Handler,
    middlewares: Vec<Middleware>,
}

type MatchedRoute = (String, RouteEntry, Vec<(String, String)>);

/// Tipo que representa o handler global de erros.
type ErrorHandler = Arc<dyn Fn(&HandlerError, &crate::RequestContext) -> Response + Send + Sync>;

/// Tipo que representa um middleware da cadeia de execução.
pub(crate) type Middleware = Arc<dyn Fn(&Request) -> MiddlewareResult + Send + Sync>;

type RouteKey = (Method, String);

enum BodyReadError {
    TooLarge,
    Read,
}

#[derive(Clone)]
struct SessionState {
    manager: Arc<SessionManager>,
    config: SessionConfig,
}

/// Responsável por guardar rotas, processar padrões e disparar middlewares.
///
/// A estrutura é interna ao framework e funciona como o componente central de
/// roteamento. Cada rota é armazenada por método e caminho, e a correspondência
/// é executada no momento em que cada pedido HTTP chega ao servidor.
#[derive(Clone, Default)]
pub(crate) struct Router {
    routes: HashMap<RouteKey, RouteEntry>,
    middlewares: Vec<Middleware>,
    error_handler: Option<ErrorHandler>,

    session: Option<SessionState>,

    logger: Logger,
}

impl Router {
    /// Cria um router vazio.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Activa a gestão de sessões no router.
    pub(crate) fn enable_sessions(&mut self, config: SessionConfig) {
        let manager =
            SessionManager::with_ttl(MemoryStore::new(), Duration::from_secs(config.max_age));

        self.session = Some(SessionState {
            manager: Arc::new(manager),
            config,
        });
    }

    /// Regista uma nova rota no router.
    ///
    /// O path pode conter segmentos dinâmicos com o prefixo `:`, por exemplo
    /// `/utilizadores/:id`.
    pub(crate) fn add<F>(&mut self, method: Method, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.add_fallible(method, path, move |request, response| {
            Ok(handler(request, response))
        })
    }

    /// Regista uma rota cujo handler pode devolver um erro.
    pub(crate) fn add_fallible<F>(
        &mut self,
        method: Method,
        path: &str,
        handler: F,
    ) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, HandlerError> + Send + Sync + 'static,
    {
        Self::validate_route(path)?;

        self.routes.insert(
            (method, path.to_owned()),
            RouteEntry {
                handler: Arc::new(handler),
                middlewares: Vec::new(),
            },
        );

        Ok(())
    }

    /// Regista um conjunto de rotas de grupo de forma atómica.
    pub(crate) fn add_group_routes(
        &mut self,
        routes: Vec<(Method, String, Handler, Vec<Middleware>)>,
    ) -> Result<(), String> {
        let mut pending_keys = HashSet::with_capacity(routes.len());

        // Primeira fase: validar todas as rotas sem modificar o Router.
        for (method, path, _, _) in &routes {
            Self::validate_route(path)?;

            let key = (method.clone(), path.clone());

            if self.routes.contains_key(&key) {
                return Err(format!(
                    "Já existe uma rota registada para o método {method} e caminho '{path}'."
                ));
            }

            if !pending_keys.insert(key) {
                return Err(format!("A rota {method} '{path}' está duplicada no grupo."));
            }
        }

        // Segunda fase: registar todas as rotas.
        for (method, path, handler, middlewares) in routes {
            self.routes.insert(
                (method, path),
                RouteEntry {
                    handler,
                    middlewares,
                },
            );
        }

        Ok(())
    }

    /// Regista o handler global para erros das rotas.
    pub(crate) fn set_error_handler<F>(&mut self, handler: F)
    where
        F: Fn(&HandlerError, &crate::RequestContext) -> Response + Send + Sync + 'static,
    {
        self.error_handler = Some(Arc::new(handler));
    }

    /// Regista o handler global para tratar erros durante o processamento
    /// dos pedidos HTTP.
    fn handle_error(&self, error: &HandlerError, context: &crate::RequestContext) -> Response {
        self.logger.error(&format!(
            "Erro HTTP {}: {}",
            error.status(),
            error.public_message()
        ));

        if let Some(details) = error.details() {
            self.logger
                .error(&format!("Detalhes técnicos: {}", details));
        }

        let mut source = std::error::Error::source(error);

        while let Some(cause) = source {
            self.logger.error(&format!("Causa original: {}", cause));

            source = std::error::Error::source(cause);
        }

        match &self.error_handler {
            Some(error_handler) => {
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    error_handler(error, context)
                })) {
                    Ok(response) => response,

                    Err(payload) => {
                        let message = extract_panic_message(payload.as_ref());

                        self.logger.error(&format!(
                            "O Global Error Handler entrou em panic: {}. \
                            Será utilizada a resposta de erro padrão.",
                            message
                        ));

                        self.error_response(500)
                    }
                }
            }

            None => self.error_response(error.status()),
        }
    }

    /// Compara um padrão de rota com o caminho da requisição.
    ///
    /// Quando encontra um marcador `:nome`, extrai o valor correspondente e
    /// devolve uma lista de pares `(nome, valor)` para uso em `Request::param`.
    fn match_path(route: &str, path: &str) -> Option<Vec<(String, String)>> {
        // Rejeita segmentos vazios no interior dos caminhos.
        if route.contains("//") || path.contains("//") {
            return None;
        }

        let route_parts: Vec<&str> = route.trim_matches('/').split('/').collect();

        let path_parts: Vec<&str> = path.trim_matches('/').split('/').collect();

        if route_parts.len() != path_parts.len() {
            return None;
        }

        let mut params = Vec::new();

        for (route_part, path_part) in route_parts.iter().zip(path_parts.iter()) {
            if let Some(name) = route_part.strip_prefix(':') {
                // O parâmetro deve ter um nome e um valor.
                if name.is_empty() || path_part.is_empty() {
                    return None;
                }

                // Não permite nomes de parâmetros repetidos.
                if params
                    .iter()
                    .any(|(existing_name, _)| existing_name == name)
                {
                    return None;
                }

                params.push((name.to_string(), (*path_part).to_string()));
            } else if route_part != path_part {
                return None;
            }
        }

        Some(params)
    }

    /// Define a prioridade de uma rota.
    /// Segmentos estáticos têm prioridade sobre segmentos dinâmicos.
    fn route_specificity(route: &str) -> Vec<u8> {
        route
            .trim_matches('/')
            .split('/')
            .map(|segment| {
                if segment.starts_with(':') {
                    0 // Segmento dinâmico
                } else {
                    1 // Segmento estático
                }
            })
            .collect()
    }

    fn path_exists(&self, path: &str) -> bool {
        self.routes
            .keys()
            .any(|(_, route_path)| Self::match_path(route_path, path).is_some())
    }

    fn allowed_methods(&self, path: &str) -> Vec<Method> {
        let mut methods: Vec<Method> = self
            .routes
            .keys()
            .filter_map(|(method, route_path)| {
                Self::match_path(route_path, path).map(|_| method.clone())
            })
            .collect();

        if methods.contains(&Method::GET) && !methods.contains(&Method::HEAD) {
            methods.push(Method::HEAD);
        }

        methods.sort_by_key(|method| method.as_str().to_owned());
        methods.dedup();

        methods
    }

    fn find_route(&self, method: &Method, path: &str) -> Option<MatchedRoute> {
        // Primeiro, procura uma correspondência exacta do método.
        let exact_match = self
            .routes
            .iter()
            .filter_map(|((route_method, route_path), handler)| {
                if route_method != method {
                    return None;
                }

                Self::match_path(route_path, path)
                    .map(|params| (route_path.clone(), handler.clone(), params))
            })
            .max_by(|(path_a, _, _), (path_b, _, _)| {
                Self::route_specificity(path_a)
                    .cmp(&Self::route_specificity(path_b))
                    .then_with(|| path_a.cmp(path_b))
            });

        if exact_match.is_some() {
            return exact_match;
        }

        // Se não existir uma rota HEAD explícita, tenta uma rota GET.
        if *method == Method::HEAD {
            return self
                .routes
                .iter()
                .filter_map(|((route_method, route_path), handler)| {
                    if *route_method != Method::GET {
                        return None;
                    }

                    Self::match_path(route_path, path)
                        .map(|params| (route_path.clone(), handler.clone(), params))
                })
                .max_by(|(path_a, _, _), (path_b, _, _)| {
                    Self::route_specificity(path_a)
                        .cmp(&Self::route_specificity(path_b))
                        .then_with(|| path_a.cmp(path_b))
                });
        }

        None
    }

    fn error_response(&self, code: u16) -> Response {
        let mut context = TemplateContext::new();

        let (title, page_path) = match code {
            404 => ("404 - Página não encontrada", "views/errors/404.html"),
            405 => ("405 - Método não permitido", "views/errors/405.html"),
            _ => ("500 - Erro interno do servidor", "views/errors/500.html"),
        };

        context.set("titulo", title.into());

        Response::error_page(
            code,
            page_path,
            "views/layouts/main.html",
            "views/partials",
            &context,
        )
    }

    /// Processa uma requisição HTTP e devolve a resposta correspondente.
    ///
    /// A lógica faz as seguintes etapas:
    /// 1. obtém método e caminho;
    /// 2. procura uma rota compatível;
    /// 3. extrai parâmetros da rota;
    /// 4. executa middleware;
    /// 5. chama o handler ou devolve 404.
    pub(crate) async fn handle(
        &self,
        request: HyperRequest<Incoming>,
    ) -> HyperResponse<Full<Bytes>> {
        let (parts, incoming_body) = request.into_parts();

        let method = parts.method.clone();
        let path = parts.uri.path().to_owned();

        let matched = self
            .find_route(&method, &path)
            .map(|(_, entry, params)| (entry, params));

        let params = match &matched {
            Some((_, params)) => params.clone(),
            None => Vec::new(),
        };

        // Extrai o identificador antes de transferir a requisição HTTP.
        let session_id = self.extract_session_id(&parts.headers);

        // Cria o Request com os metadados e um corpo inicialmente vazio.
        let inner = HyperRequest::from_parts(parts, Bytes::new());
        let mut request = Request::new(inner, params);
        let context = request.context();

        // Carrega a sessão.
        let session = match self.load_session(session_id.as_deref()) {
            Ok(session) => session,

            Err(error) => {
                let handler_error = HandlerError::with_status_and_details(
                    500,
                    "Não foi possível carregar a sessão.",
                    error.to_string(),
                )
                .with_source(error);

                return self.handle_error(&handler_error, &context).into_hyper();
            }
        };

        if let Some(session) = session {
            request.set_session(session);
        }

        if let Some(session) = &self.session {
            request.set_session_manager(Arc::clone(&session.manager));
        }

        // Lemos o corpo antes dos middlewares para permitir que eles
        // analisem dados da requisição, como tokens CSRF enviados por formulário.
        if matched.is_some() {
            match Self::read_body(incoming_body).await {
                Ok(body) => {
                    request.set_body(body);
                }

                Err(body_error) => {
                    let (status, message) = match body_error {
                        BodyReadError::TooLarge => {
                            (413, "O corpo do pedido excede o tamanho máximo permitido.")
                        }
                        BodyReadError::Read => (400, "Não foi possível ler o corpo do pedido."),
                    };

                    let error = HandlerError::with_status(status, message);
                    let response = self.handle_error(&error, &context);

                    let mut response =
                        self.finalize_or_error_response(response, request.session(), &context);

                    if method == Method::HEAD {
                        *response.body_mut() = Full::new(Bytes::new());
                    }

                    return response;
                }
            }
        }

        let route_middlewares = matched
            .as_ref()
            .map(|(entry, _)| entry.middlewares.as_slice())
            .unwrap_or(&[]);

        for middleware in self.middlewares.iter().chain(route_middlewares.iter()) {
            let middleware_result =
                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    middleware(&request)
                })) {
                    Ok(result) => result,

                    Err(payload) => {
                        let message = extract_panic_message(payload.as_ref());

                        MiddlewareResult::Error(HandlerError::with_details(
                            "Ocorreu um erro interno no servidor.",
                            format!("Panic no middleware: {}", message),
                        ))
                    }
                };

            match middleware_result {
                MiddlewareResult::Next => {}

                MiddlewareResult::Respond(response) => {
                    let mut response =
                        self.finalize_or_error_response(response, request.session(), &context);

                    if method == Method::HEAD {
                        *response.body_mut() = Full::new(Bytes::new());
                    }

                    return response;
                }

                MiddlewareResult::Error(error) => {
                    let response = self.handle_error(&error, &context);

                    let mut response =
                        self.finalize_or_error_response(response, request.session(), &context);

                    if method == Method::HEAD {
                        *response.body_mut() = Full::new(Bytes::new());
                    }

                    return response;
                }
            }
        }

        let mut response = match matched {
            Some((entry, _)) => {
                let handler = entry.handler;
                // Guardamos a sessão antes de mover Request para o handler.
                let session = request.session();

                match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handler(request, Response::new())
                })) {
                    Ok(Ok(response)) => {
                        self.finalize_or_error_response(response, session, &context)
                    }

                    Ok(Err(error)) => {
                        let response = self.handle_error(&error, &context);

                        self.finalize_or_error_response(response, session, &context)
                    }

                    Err(payload) => {
                        let message = extract_panic_message(payload.as_ref());

                        let error = HandlerError::with_details(
                            "Ocorreu um erro interno no servidor.",
                            format!("Panic no handler da rota: {}", message),
                        );

                        let response = self.handle_error(&error, &context);

                        self.finalize_or_error_response(response, session, &context)
                    }
                }
            }
            None if self.path_exists(&path) => {
                let allow = self
                    .allowed_methods(&path)
                    .iter()
                    .map(Method::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");

                let error = HandlerError::with_status(405, "Método não permitido");

                let response = self.handle_error(&error, &context).header("Allow", &allow);

                self.finalize_or_error_response(response, request.session(), &context)
            }

            None => {
                let error = HandlerError::with_status(404, "Rota não encontrada");

                let response = self.handle_error(&error, &context);

                self.finalize_or_error_response(response, request.session(), &context)
            }
        };
        if method == Method::HEAD {
            *response.body_mut() = Full::new(Bytes::new());
        }

        response
    }

    /// Adiciona um middleware ao router.
    ///
    /// Middleware globais são executados antes da rota final e permitem tarefas
    /// como autenticação, logging, validação de acessos e interceptação de
    /// requisições.
    pub(crate) fn use_middleware<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> Option<Response> + Send + Sync + 'static,
    {
        self.middlewares
            .push(Arc::new(move |request| match middleware(request) {
                Some(response) => MiddlewareResult::Respond(response),
                None => MiddlewareResult::Next,
            }));
    }

    pub(crate) fn use_middleware_advanced<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> MiddlewareResult + Send + Sync + 'static,
    {
        self.middlewares.push(Arc::new(middleware));
    }

    pub(crate) fn validate_route(path: &str) -> Result<(), String> {
        if path.is_empty() {
            return Err("O caminho da rota não pode estar vazio.".to_string());
        }

        if path.contains("//") {
            return Err("A rota não pode conter segmentos vazios.".to_string());
        }

        let segments: Vec<&str> = path.trim_matches('/').split('/').collect();
        let mut param_names = std::collections::HashSet::new();

        for segment in segments {
            if let Some(name) = segment.strip_prefix(':') {
                if name.is_empty() {
                    return Err("Os parâmetros dinâmicos devem ter um nome.".to_string());
                }

                if !param_names.insert(name) {
                    return Err(format!("O parâmetro ':{}' está repetido na rota.", name));
                }
            }
        }

        Ok(())
    }

    fn resolve_session(
        &self,
        session_id: Option<&str>,
    ) -> Result<Option<crate::session::Session>, crate::session::SessionStoreError> {
        let Some(session) = self.session.as_ref() else {
            return Ok(None);
        };

        match session_id {
            Some(id) => session.manager.load(id).map(Some),
            None => Ok(Some(session.manager.create())),
        }
    }

    fn extract_session_id(&self, headers: &http::HeaderMap) -> Option<String> {
        let config = &self.session.as_ref()?.config;

        headers
            .get_all(hyper::header::COOKIE)
            .iter()
            .map(|header| {
                header
                    .to_str()
                    .map_err(|_| crate::session::cookie::CookieError::InvalidValue)
            })
            .collect::<Result<Vec<_>, _>>()
            .and_then(|headers| Cookie::get_unique_from_headers(headers, &config.cookie_name))
            .ok()
            .flatten()
    }

    /// Recupera a sessão associada a um pedido HTTP.
    fn load_session(
        &self,
        session_id: Option<&str>,
    ) -> Result<Option<Arc<Mutex<crate::session::Session>>>, crate::session::SessionStoreError>
    {
        let Some(session) = self.resolve_session(session_id)? else {
            return Ok(None);
        };

        Ok(Some(Arc::new(Mutex::new(session))))
    }
    /// Finaliza a resposta HTTP e persiste a sessão, quando necessário.
    fn finalize_response(
        &self,
        mut response: Response,
        session: Option<Arc<Mutex<crate::session::Session>>>,
    ) -> Result<HyperResponse<Full<Bytes>>, Box<(crate::session::SessionError, Response)>> {
        let Some(session) = session else {
            return Ok(response.into_hyper());
        };

        let session = match session.lock() {
            Ok(session) => session,
            Err(_) => {
                return Err(Box::new((
                    crate::session::SessionError::LockPoisoned,
                    response,
                )));
            }
        };

        let session_state = match self.session.as_ref() {
            Some(session_state) => session_state,
            None => {
                return Err(Box::new((
                    crate::session::SessionError::Configuration,
                    response,
                )));
            }
        };

        let manager = &session_state.manager;
        let config = &session_state.config;

        if session.is_destroyed() {
            if let Err(error) = manager.destroy(session.id()) {
                return Err(Box::new((error.into(), response)));
            }

            if let Err(error) = response.remove_cookie(&config.cookie_name, config) {
                return Err(Box::new((error.into(), response)));
            }

            return Ok(response.into_hyper());
        }

        if session.is_modified() {
            let cookie = Cookie::new(config.cookie_name.clone(), session.id().to_string());

            if let Err(error) = response.cookie(&cookie, config) {
                return Err(Box::new((error.into(), response)));
            }

            if let Err(error) = manager.save(&session) {
                return Err(Box::new((error.into(), response)));
            }
        }

        Ok(response.into_hyper())
    }

    fn finalize_or_error_response(
        &self,
        response: Response,
        session: Option<Arc<Mutex<crate::session::Session>>>,
        context: &crate::RequestContext,
    ) -> HyperResponse<Full<Bytes>> {
        match self.finalize_response(response, session) {
            Ok(response) => response,

            Err(error) => {
                let (error, _response) = *error;
                let handler_error = HandlerError::with_status_and_details(
                    500,
                    "Ocorreu um erro interno no servidor.",
                    error.to_string(),
                )
                .with_source(error);

                self.handle_error(&handler_error, context).into_hyper()
            }
        }
    }

    pub(crate) fn set_logger(&mut self, logger: Logger) {
        self.logger = logger;
    }

    async fn read_body(mut body: Incoming) -> Result<Bytes, BodyReadError> {
        let mut buffer = BytesMut::new();

        while let Some(frame) = body.frame().await {
            let frame = frame.map_err(|_| BodyReadError::Read)?;

            if let Some(data) = frame.data_ref() {
                if data.len() > MAX_BODY_SIZE.saturating_sub(buffer.len()) {
                    return Err(BodyReadError::TooLarge);
                }

                buffer.extend_from_slice(data);
            }
        }

        Ok(buffer.freeze())
    }
}

fn extract_panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(message) = payload.downcast_ref::<String>() {
        message.as_str()
    } else if let Some(message) = payload.downcast_ref::<&'static str>() {
        message
    } else {
        "O panic contém um payload desconhecido."
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionConfig;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use std::sync::{Arc, Mutex};

    fn create_test_context() -> crate::RequestContext {
        crate::RequestContext::test()
    }

    #[test]
    fn matches_static_route() {
        let params = Router::match_path("/produtos/novo", "/produtos/novo");

        assert!(params.is_some());
        assert!(params.unwrap().is_empty());
    }

    #[test]
    fn extracts_dynamic_route_params() {
        let params = Router::match_path("/produtos/:id", "/produtos/42")
            .expect("A rota deveria corresponder");

        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "id");
        assert_eq!(params[0].1, "42");
    }

    #[test]
    fn rejects_route_with_extra_segments() {
        let params = Router::match_path("/produtos/:id", "/produtos/42/detalhes");

        assert!(params.is_none());
    }

    #[test]
    fn static_route_has_priority_over_dynamic_route() {
        let static_specificity = Router::route_specificity("/produtos/novo");
        let dynamic_specificity = Router::route_specificity("/produtos/:id");

        assert!(
            static_specificity > dynamic_specificity,
            "A rota estática deve ter prioridade sobre a dinâmica"
        );
    }

    #[test]
    fn selects_static_route_over_dynamic_route() {
        let mut router = Router::new();

        let _ = router.add(Method::GET, "/produtos/:id", |_, response| {
            response.send("dinamica")
        });

        let _ = router.add(Method::GET, "/produtos/novo", |_, response| {
            response.send("estatica")
        });

        let (selected_path, _, params) = router
            .find_route(&Method::GET, "/produtos/novo")
            .expect("A rota deveria ser encontrada");

        assert_eq!(selected_path, "/produtos/novo");
        assert!(params.is_empty());
    }

    #[test]
    fn prefers_static_segment_at_the_end() {
        let mut router = Router::new();

        let _ = router.add(Method::GET, "/utilizadores/:id/:secao", |_, response| {
            response.send("dinamica")
        });

        let _ = router.add(Method::GET, "/utilizadores/:id/perfil", |_, response| {
            response.send("perfil")
        });

        let (selected_path, _, params) = router
            .find_route(&Method::GET, "/utilizadores/42/perfil")
            .expect("A rota deveria ser encontrada");

        assert_eq!(selected_path, "/utilizadores/:id/perfil");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], ("id".to_string(), "42".to_string()));
    }

    #[test]
    fn resolves_equal_specificity_deterministically() {
        let mut router = Router::new();

        let _ = router.add(Method::GET, "/a/:x/c", |_, response| {
            response.send("rota_a")
        });

        let _ = router.add(Method::GET, "/a/b/:x", |_, response| {
            response.send("rota_b")
        });

        let (selected_path, _, _) = router
            .find_route(&Method::GET, "/a/b/c")
            .expect("Uma rota deveria ser encontrada");

        // Ambas têm dois segmentos estáticos.
        // O desempate é feito pela ordem lexicográfica do caminho.
        assert_eq!(selected_path, "/a/b/:x");
    }

    #[test]
    fn rejects_empty_segments_in_path() {
        let params = Router::match_path("/utilizadores/:id/perfil", "/utilizadores//perfil");

        assert!(params.is_none());
    }

    #[test]
    fn rejects_empty_dynamic_parameter_values() {
        let params = Router::match_path("/utilizadores/:id", "/utilizadores/");

        assert!(params.is_none());
    }

    #[test]
    fn rejects_duplicate_parameter_names() {
        let params = Router::match_path(
            "/utilizadores/:id/documentos/:id",
            "/utilizadores/10/documentos/20",
        );

        assert!(params.is_none());
    }

    #[test]
    fn accepts_valid_route_definition() {
        assert!(Router::validate_route("/utilizadores/:id").is_ok());
    }

    #[test]
    fn rejects_route_without_path() {
        assert!(Router::validate_route("").is_err());
    }

    #[test]
    fn rejects_route_with_empty_segment() {
        assert!(Router::validate_route("/utilizadores//perfil").is_err());
    }

    #[test]
    fn rejects_route_with_empty_parameter_name() {
        assert!(Router::validate_route("/utilizadores/:").is_err());
    }

    #[test]
    fn rejects_route_with_duplicate_parameter_names() {
        assert!(Router::validate_route("/utilizadores/:id/documentos/:id").is_err());
    }

    #[test]
    fn returns_error_when_registering_invalid_route() {
        let mut router = Router::new();

        let result = router.add(Method::GET, "/utilizadores/:", |_, response| {
            response.send("inválida")
        });

        assert!(result.is_err());
    }

    #[test]
    fn registers_valid_route_successfully() {
        let mut router = Router::new();

        let _ = router.add(Method::GET, "/utilizadores/:id", |_, response| {
            response.send("válida")
        });

        assert!(
            router
                .find_route(&Method::GET, "/utilizadores/42")
                .is_some()
        );
    }

    #[test]
    fn detects_path_registered_with_another_method() {
        let mut router = Router::new();

        router
            .add(Method::GET, "/utilizadores", |_, response| {
                response.send("Lista de utilizadores")
            })
            .expect("A rota GET deveria ser registada");

        router
            .add(Method::POST, "/utilizadores", |_, response| {
                response.send("Utilizador criado")
            })
            .expect("A rota POST deveria ser registada");

        let get_route = router.find_route(&Method::GET, "/utilizadores");
        let post_route = router.find_route(&Method::POST, "/utilizadores");
        let put_route = router.find_route(&Method::PUT, "/utilizadores");

        assert!(get_route.is_some());
        assert!(post_route.is_some());
        assert!(put_route.is_none());

        let methods = router.allowed_methods("/utilizadores");

        assert_eq!(methods, vec![Method::GET, Method::HEAD, Method::POST]);
    }

    #[test]
    fn prefers_explicit_head_route_over_get_route() {
        let mut router = Router::new();

        router
            .add(Method::GET, "/utilizadores", |_, response| {
                response.send("Resposta GET")
            })
            .expect("A rota GET deveria ser registada");

        router
            .add(Method::HEAD, "/utilizadores", |_, response| {
                response.send("Resposta HEAD")
            })
            .expect("A rota HEAD deveria ser registada");

        let (_, entry, _) = router
            .find_route(&Method::HEAD, "/utilizadores")
            .expect("Deveria encontrar a rota HEAD");

        // Compara a identidade dos handlers registados.
        let head_entry = router
            .routes
            .get(&(Method::HEAD, "/utilizadores".to_owned()))
            .expect("A rota HEAD deveria existir");

        assert!(
            std::sync::Arc::ptr_eq(&entry.handler, &head_entry.handler),
            "O router deveria preferir o handler HEAD explícito"
        );
    }

    #[test]
    fn detects_allowed_methods_for_dynamic_route() {
        let mut router = Router::new();

        router
            .add(Method::GET, "/utilizadores/:id", |_, response| {
                response.send("Detalhes do utilizador")
            })
            .expect("A rota GET deveria ser registada");

        router
            .add(Method::DELETE, "/utilizadores/:id", |_, response| {
                response.send("Utilizador eliminado")
            })
            .expect("A rota DELETE deveria ser registada");

        let methods = router.allowed_methods("/utilizadores/42");

        assert_eq!(methods, vec![Method::DELETE, Method::GET, Method::HEAD]);
    }

    #[test]
    fn middleware_none_allows_handler_to_continue() {
        let mut router = Router::new();

        router.use_middleware(|_| None);

        router
            .add(Method::GET, "/teste", |_, response| {
                response.send("Handler executado")
            })
            .expect("A rota deveria ser registada");

        // Neste módulo, podemos testar directamente o registo
        // do middleware, mas handle() exige uma requisição Hyper
        // com um corpo Incoming.
    }

    #[tokio::test]
    async fn finalize_response_uses_global_handler_when_cookie_fails() {
        let mut router = Router::new();

        let mut config = SessionConfig::default();
        config.path.clear();

        router.enable_sessions(config);

        let manager = Arc::clone(&router.session.as_ref().unwrap().manager);

        let mut session = manager.create();
        session.insert("user", "Fabrício").unwrap();

        let session_id = session.id().to_string();
        let session = Arc::new(Mutex::new(session));

        // Guarda os dados recebidos pelo handler global.
        let captured = Arc::new(Mutex::new(None));
        let captured_handler = Arc::clone(&captured);

        router.set_error_handler(move |error, context| {
            *captured_handler.lock().unwrap() = Some((
                error.status(),
                error.details().map(str::to_owned),
                std::error::Error::source(error).is_some(),
                context.path().to_string(),
            ));

            Response::new().status(500).send("Erro tratado globalmente")
        });

        let response = Response::new()
            .status(403)
            .header("X-Test", "original")
            .send("Acesso negado");

        let context = create_test_context();

        let hyper_response = router.finalize_or_error_response(response, Some(session), &context);

        // A resposta deve ser a produzida pelo handler global.
        assert_eq!(hyper_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = hyper_response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();

        assert_eq!(bytes.as_ref(), "Erro tratado globalmente".as_bytes());

        // Verifica os dados recebidos pelo handler global.
        let captured = captured.lock().unwrap();
        let (status, details, has_source, path) = captured
            .as_ref()
            .expect("O handler global deveria ser chamado");

        assert_eq!(*status, 500);
        assert!(details.is_some());
        assert!(*has_source);
        assert_eq!(path, "/teste");

        // A sessão não deve ser guardada quando a criação do cookie falha.
        let stored_session = manager.load(&session_id).unwrap();

        assert!(!stored_session.contains("user"));
    }

    #[tokio::test]
    async fn finalize_response_uses_global_handler_when_removal_cookie_fails() {
        let mut router = Router::new();

        let mut config = SessionConfig::default();
        config.path.clear();

        router.enable_sessions(config);

        let manager = Arc::clone(&router.session.as_ref().unwrap().manager);

        let mut session = manager.create();
        session.insert("user", "Fabrício").unwrap();

        let session_id = session.id().to_string();

        session.destroy();

        let session = Arc::new(Mutex::new(session));

        let captured = Arc::new(Mutex::new(None));
        let captured_handler = Arc::clone(&captured);

        router.set_error_handler(move |error, context| {
            *captured_handler.lock().unwrap() = Some((
                error.status(),
                error.details().map(str::to_owned),
                std::error::Error::source(error).is_some(),
                context.path().to_string(),
            ));

            Response::new().status(500).send("Erro tratado globalmente")
        });

        let response = Response::new().status(401).send("Sessão inválida");

        let context = create_test_context();

        let hyper_response = router.finalize_or_error_response(response, Some(session), &context);

        assert_eq!(hyper_response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = hyper_response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();

        assert_eq!(bytes.as_ref(), "Erro tratado globalmente".as_bytes());

        let captured = captured.lock().unwrap();
        let (status, details, has_source, path) = captured
            .as_ref()
            .expect("O handler global deveria ser chamado");

        assert_eq!(*status, 500);
        assert!(details.is_some());
        assert!(*has_source);
        assert_eq!(path, "/teste");

        // A sessão destruída deve continuar removida do armazenamento.
        let stored_session = manager.load(&session_id).unwrap();

        assert!(!stored_session.contains("user"));
    }

    #[test]
    fn creates_new_session_when_session_id_is_missing() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let session = router
            .resolve_session(None)
            .unwrap()
            .expect("Deveria criar uma nova sessão");

        assert!(!session.id().is_empty());
        assert!(!session.is_modified());
    }

    #[test]
    fn creates_new_session_when_session_id_is_destroyed() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let manager = Arc::clone(&router.session.as_ref().unwrap().manager);

        let mut original = manager.create();
        original.insert("user", "Fabrício").unwrap();

        let old_id = original.id().to_string();

        manager.save(&original).unwrap();
        manager.destroy(&old_id).unwrap();

        let resolved = router
            .resolve_session(Some(&old_id))
            .unwrap()
            .expect("Deveria criar uma nova sessão após a destruição");

        assert_ne!(resolved.id(), old_id);
        assert!(!resolved.contains("user"));
    }

    #[test]
    fn creates_new_session_when_session_id_has_expired() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let manager = Arc::clone(&router.session.as_ref().unwrap().manager);

        let original = manager.create();
        let old_id = original.id().to_string();

        let resolved = router
            .resolve_session(Some(&old_id))
            .unwrap()
            .expect("Deveria criar uma nova sessão após a expiração");

        assert_ne!(resolved.id(), old_id);
    }

    #[test]
    fn extracts_session_id_from_headers() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();

        headers.insert(
            hyper::header::COOKIE,
            "theme=dark; rustpress_session=abc123".parse().unwrap(),
        );

        let session_id = router.extract_session_id(&headers);

        assert_eq!(session_id, Some("abc123".to_string()));
    }

    #[test]
    fn rejects_duplicate_session_cookies_in_same_header() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();

        headers.insert(
            hyper::header::COOKIE,
            "rustpress_session=abc123; rustpress_session=xyz789"
                .parse()
                .unwrap(),
        );

        let session_id = router.extract_session_id(&headers);

        assert!(session_id.is_none());
    }

    #[test]
    fn rejects_duplicate_session_cookies_across_headers() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();

        headers.append(
            hyper::header::COOKIE,
            "rustpress_session=abc123".parse().unwrap(),
        );

        headers.append(
            hyper::header::COOKIE,
            "rustpress_session=xyz789".parse().unwrap(),
        );

        let session_id = router.extract_session_id(&headers);

        assert!(session_id.is_none());
    }

    #[test]
    fn rejects_invalid_cookie_header_encoding() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();

        let invalid_value = http::HeaderValue::from_bytes(&[
            b'r', b'u', b's', b't', b'=', b'd', b'a', b'r', b'k', 0xFF,
        ])
        .unwrap();

        headers.append(hyper::header::COOKIE, invalid_value);

        let session_id = router.extract_session_id(&headers);

        assert!(session_id.is_none());
    }

    #[test]
    fn returns_none_when_session_cookie_is_missing() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();
        headers.insert(
            hyper::header::COOKIE,
            "theme=dark; language=pt".parse().unwrap(),
        );

        assert!(router.extract_session_id(&headers).is_none());
    }

    #[test]
    fn rejects_session_cookie_without_equals_sign_in_router() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();
        headers.insert(hyper::header::COOKIE, "rustpress_session".parse().unwrap());

        assert!(router.extract_session_id(&headers).is_none());
    }

    #[test]
    fn rejects_invalid_session_cookie_value_in_router() {
        let mut router = Router::new();
        router.enable_sessions(SessionConfig::default());

        let mut headers = http::HeaderMap::new();
        headers.insert(
            hyper::header::COOKIE,
            "rustpress_session=bad value".parse().unwrap(),
        );

        assert!(router.extract_session_id(&headers).is_none());
    }

    #[test]
    fn returns_none_when_sessions_are_disabled() {
        let router = Router::new();

        let mut headers = http::HeaderMap::new();
        headers.insert(
            hyper::header::COOKIE,
            "rustpress_session=550e8400-e29b-41d4-a716-446655440000"
                .parse()
                .unwrap(),
        );

        assert!(router.extract_session_id(&headers).is_none());
    }

    #[test]
    fn global_error_handler_panic_uses_default_response() {
        let mut router = Router::new();

        router.set_error_handler(|_, _| {
            panic!("Falha simulada no Global Error Handler");
        });

        let context = create_test_context();

        let error = HandlerError::new("Erro original");

        let response = router.handle_error(&error, &context);

        assert_eq!(response.status_code(), 500);
    }

    #[test]
    fn extracts_string_panic_message() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(String::from("Erro em String"));

        assert_eq!(extract_panic_message(payload.as_ref()), "Erro em String");
    }

    #[test]
    fn extracts_static_str_panic_message() {
        let payload: Box<dyn std::any::Any + Send> = Box::new("Erro em &str");

        assert_eq!(extract_panic_message(payload.as_ref()), "Erro em &str");
    }

    #[test]
    fn handles_unknown_panic_payload() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(123);

        assert_eq!(
            extract_panic_message(payload.as_ref()),
            "O panic contém um payload desconhecido."
        );
    }
}
