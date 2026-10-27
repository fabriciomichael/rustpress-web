use std::{collections::HashMap, sync::Arc};

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::{Request as HyperRequest, Response as HyperResponse, body::Incoming, http::Method};

use crate::{TemplateContext, request::Request, response::Response};

/// Tipo que representa um handler de rota.
///
/// Um handler recebe a requisição processada pelo framework e a resposta em
/// construção e devolve a resposta final a enviar ao cliente.
type Handler = Arc<dyn Fn(Request, Response) -> Response + Send + Sync>;

/// Tipo que representa um middleware de pipeline.
///
/// Middleware executa-se antes do handler principal e pode interromper o fluxo
/// devolvendo `Some(Response)`.
type Middleware = Arc<dyn Fn(&Request) -> Option<Response> + Send + Sync>;

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
}

impl Router {
    /// Cria um router vazio.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Regista uma nova rota no router.
    ///
    /// O path pode conter segmentos dinâmicos com o prefixo `:`, por exemplo
    /// `/utilizadores/:id`.

    pub(crate) fn add<F>(&mut self, method: Method, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        Self::validate_route(path)?;

        self.routes
            .insert((method, path.to_owned()), Arc::new(handler));

        Ok(())
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

    fn find_route(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(String, Handler, Vec<(String, String)>)> {
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

        let request = Request::new(request, params);

        for middleware in &self.middlewares {
            if let Some(response) = middleware(&request) {
                let mut response = response.into_hyper();

                if method == Method::HEAD {
                    *response.body_mut() = Full::new(Bytes::new());
                }

                return response;
            }
        }

        let mut response = match matched {
            Some((handler, _)) => handler(request, Response::new()).into_hyper(),

            None if self.path_exists(&path) => {
                let allow = self
                    .allowed_methods(&path)
                    .iter()
                    .map(Method::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");

                self.error_response(405)
                    .header("Allow", &allow)
                    .into_hyper()
            }

            None => self.error_response(404).into_hyper(),
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
