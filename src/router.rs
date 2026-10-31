//! Correspondência de rotas e despacho de pedidos.
//!
//! O router é um componente interno: armazena handlers por método e padrão
//! de caminho, extrai parâmetros dinâmicos, executa middlewares e encaminha
//! erros para o handler global configurado em [`crate::App`].
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming, http::Method};

use crate::{
    HandlerError, MiddlewareResult, TemplateContext,
    request::Request,
    response::Response,
    session::{Cookie, MemoryStore, SessionConfig, SessionManager},
};

/// Tipo que representa um handler de rota.
///
/// Um handler recebe a requisição processada pelo framework e a resposta em
/// construção e devolve a resposta final a enviar ao cliente.
type Handler = Arc<dyn Fn(Request, Response) -> Result<Response, HandlerError> + Send + Sync>;
type MatchedRoute = (String, Handler, Vec<(String, String)>);

/// Tipo que representa o handler global de erros.
type ErrorHandler = Arc<dyn Fn(&HandlerError, &crate::RequestContext) -> Response + Send + Sync>;

/// Tipo que representa um middleware da cadeia de execução.
type Middleware = Arc<dyn Fn(&Request) -> MiddlewareResult + Send + Sync>;

type RouteKey = (Method, String);

/// Responsável por guardar rotas, processar padrões e disparar middlewares.
///
/// A estrutura é interna ao framework e funciona como o componente central de
/// roteamento. Cada rota é armazenada por método e caminho, e a correspondência
/// é executada no momento em que cada pedido HTTP chega ao servidor.
#[derive(Clone, Default)]
pub(crate) struct Router {
    routes: HashMap<RouteKey, Handler>,
    middlewares: Vec<Middleware>,
    error_handler: Option<ErrorHandler>,

    session_manager: Option<Arc<SessionManager>>,
    session_config: Option<SessionConfig>,
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

        self.session_manager = Some(Arc::new(manager));
        self.session_config = Some(config);
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

        self.routes
            .insert((method, path.to_owned()), Arc::new(handler));

        Ok(())
    }

    /// Regista o handler global para erros das rotas.
    pub(crate) fn set_error_handler<F>(&mut self, handler: F)
    where
        F: Fn(&HandlerError, &crate::RequestContext) -> Response + Send + Sync + 'static,
    {
        self.error_handler = Some(Arc::new(handler));
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
    pub(crate) fn handle(&self, request: HyperRequest<Incoming>) -> HyperResponse<Full<Bytes>> {
        let method = request.method().clone();
        let path = request.uri().path().to_owned();

        let matched = self
            .find_route(&method, &path)
            .map(|(_, handler, params)| (handler, params));

        let params = match &matched {
            Some((_, params)) => params.clone(),
            None => Vec::new(),
        };

        let session = self.load_session(&request);

        let mut request = Request::new(request, params);

        if let Some(session) = session {
            request.set_session(session);
        }

        if let Some(manager) = self.session_manager.clone() {
            request.set_session_manager(manager);
        }

        let context = request.context();

        for middleware in &self.middlewares {
            match middleware(&request) {
                MiddlewareResult::Next => {}

                MiddlewareResult::Respond(response) => {
                    let mut response = self.finalize_response(response, request.session());

                    if method == Method::HEAD {
                        *response.body_mut() = Full::new(Bytes::new());
                    }

                    return response;
                }

                MiddlewareResult::Error(error) => {
                    let response = match &self.error_handler {
                        Some(error_handler) => error_handler(&error, &context),
                        None => self.error_response(error.status()),
                    };

                    let mut response = self.finalize_response(response, request.session());

                    if method == Method::HEAD {
                        *response.body_mut() = Full::new(Bytes::new());
                    }

                    return response;
                }
            }
        }

        let context = request.context();

        let mut response = match matched {
            Some((handler, _)) => {
                // Guardamos a sessão antes de mover Request para o handler.
                let session = request.session();

                match handler(request, Response::new()) {
                    Ok(response) => self.finalize_response(response, session),

                    Err(error) => {
                        let response = match &self.error_handler {
                            Some(error_handler) => error_handler(&error, &context),
                            None => {
                                eprintln!("Erro no handler: {error}");
                                self.error_response(error.status())
                            }
                        };

                        self.finalize_response(response, session)
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

                let response = match &self.error_handler {
                    Some(error_handler) => error_handler(&error, &context).header("Allow", &allow),
                    None => self.error_response(405).header("Allow", &allow),
                };

                self.finalize_response(response, request.session())
            }

            None => {
                let error = HandlerError::with_status(404, "Rota não encontrada");

                let response = match &self.error_handler {
                    Some(error_handler) => error_handler(&error, &context),
                    None => self.error_response(404),
                };

                self.finalize_response(response, request.session())
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

    fn validate_route(path: &str) -> Result<(), String> {
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

    /// Recupera a sessão associada a um pedido HTTP.
    fn load_session(
        &self,
        request: &HyperRequest<Incoming>,
    ) -> Option<Arc<Mutex<crate::session::Session>>> {
        let manager = self.session_manager.as_ref()?;
        let config = self.session_config.as_ref()?;

        let session = request
            .headers()
            .get(hyper::header::COOKIE)
            .and_then(|header| header.to_str().ok())
            .and_then(|header| Cookie::get(header, &config.cookie_name))
            .map(|session_id| manager.load(&session_id))
            .unwrap_or_else(|| manager.create());

        Some(Arc::new(Mutex::new(session)))
    }

    /// Finaliza a resposta HTTP e persiste a sessão, quando necessário.

    fn finalize_response(
        &self,
        mut response: Response,
        session: Option<Arc<Mutex<crate::session::Session>>>,
    ) -> HyperResponse<Full<Bytes>> {
        let Some(session) = session else {
            return response.into_hyper();
        };

        let (Ok(session), Some(manager), Some(config)) = (
            session.lock(),
            self.session_manager.as_ref(),
            self.session_config.as_ref(),
        ) else {
            return response.into_hyper();
        };

        if session.is_destroyed() {
            manager.destroy(session.id());

            let _ = response.remove_cookie(&config.cookie_name, config);

            return response.into_hyper();
        }

        if session.is_modified() {
            manager.save(&session);

            let cookie = Cookie::new(config.cookie_name.clone(), session.id().to_string());

            let _ = response.cookie(&cookie, config);

            return response.into_hyper();
        }

        response.into_hyper()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::SessionConfig;
    use http::StatusCode;
    use http_body_util::BodyExt;
    use hyper::body::Body;
    use std::sync::{Arc, Mutex};

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

        let (_, handler, _) = router
            .find_route(&Method::HEAD, "/utilizadores")
            .expect("Deveria encontrar a rota HEAD");

        // O Handler é um Arc. Podemos comparar a identidade do handler
        // seleccionado com o handler registado para HEAD.
        let head_handler = router
            .routes
            .get(&(Method::HEAD, "/utilizadores".to_owned()))
            .expect("A rota HEAD deveria existir");

        assert!(
            std::sync::Arc::ptr_eq(&handler, head_handler),
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
    async fn finalize_response_preserves_response_when_cookie_fails() {
        let mut router = Router::new();

        let mut config = SessionConfig::default();
        config.path.clear();

        router.enable_sessions(config);

        let manager = router.session_manager.as_ref().unwrap();

        let mut session = manager.create();
        session.insert("user", "Fabrício").unwrap();

        let session = Arc::new(Mutex::new(session));

        let response = Response::new()
            .status(403)
            .header("X-Test", "preserved")
            .send("Acesso negado");

        let hyper_response = router.finalize_response(response, Some(session));

        assert_eq!(hyper_response.status(), StatusCode::FORBIDDEN);
        assert_eq!(hyper_response.headers().get("X-Test").unwrap(), "preserved");

        assert_eq!(hyper_response.body().size_hint().exact(), Some(13));
    }

    #[tokio::test]
    async fn finalize_response_preserves_response_when_removal_cookie_fails() {
        let mut router = Router::new();

        let mut config = SessionConfig::default();
        config.path.clear();

        router.enable_sessions(config);

        let manager = router.session_manager.as_ref().unwrap();

        let mut session = manager.create();
        session.destroy();

        let session = Arc::new(Mutex::new(session));

        let response = Response::new().status(401).send("Sessão inválida");

        let hyper_response = router.finalize_response(response, Some(session));

        assert_eq!(hyper_response.status(), StatusCode::UNAUTHORIZED);

        let body = hyper_response.into_body();
        let bytes = body.collect().await.unwrap().to_bytes();

        assert_eq!(bytes.as_ref(), "Sessão inválida".as_bytes());
    }
}
