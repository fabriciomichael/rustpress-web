//! Configuração de aplicações RustPress.
//!
//! [`App`] é a fachada usada para registar rotas, middlewares, ficheiros
//! estáticos, handlers de erro e iniciar o servidor.
//!
//! Os caminhos de ficheiros são interpretados relativamente ao directório
//! de trabalho do processo.

use hyper::http::Method;

use crate::{
    MiddlewareResult, TemplateContext,
    logger::{Level, Logger},
    request::Request,
    response::Response,
    router::{Handler, Middleware, Router},
    server,
    session::{CookieError, SessionConfig},
    upload::{UploadConfig, UploadError},
};

use std::{
    fs,
    path::{Component, PathBuf},
    sync::Arc,
};

type ServerError = Box<dyn std::error::Error + Send + Sync>;

/// Representa uma rota que ainda não foi registada no Router.
struct PendingRoute {
    method: Method,
    path: String,
    handler: Handler,
    middlewares: Vec<Middleware>,
}

/// Permite configurar um conjunto de rotas com um prefixo comum.
pub struct RouteGroup {
    prefix: String,
    middlewares: Vec<Middleware>,
    routes: Vec<PendingRoute>,
}

impl RouteGroup {
    /// Junta o prefixo do grupo ao caminho da rota.
    fn join_paths(prefix: &str, path: &str) -> String {
        let prefix = prefix.trim_end_matches('/');
        let path = path.trim_start_matches('/');

        if prefix.is_empty() {
            format!("/{path}")
        } else if path.is_empty() {
            format!("{prefix}/")
        } else {
            format!("{prefix}/{path}")
        }
    }

    /// Regista temporariamente uma rota fallible no grupo.
    fn add_fallible<F>(&mut self, method: Method, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        let full_path = Self::join_paths(&self.prefix, path);

        Router::validate_route(&full_path)?;

        self.routes.push(PendingRoute {
            method,
            path: full_path,
            handler: Arc::new(handler),
            middlewares: Vec::new(),
        });

        Ok(())
    }

    /// Regista um middleware simples no grupo.
    ///
    /// O middleware será aplicado a todas as rotas do grupo.
    pub fn use_middleware<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> Option<Response> + Send + Sync + 'static,
    {
        self.middlewares
            .push(Arc::new(move |request| match middleware(request) {
                Some(response) => MiddlewareResult::Respond(response),
                None => MiddlewareResult::Next,
            }));
    }

    /// Regista um middleware avançado no grupo.
    ///
    /// Permite continuar, responder directamente ou devolver um erro.
    pub fn use_middleware_advanced<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> MiddlewareResult + Send + Sync + 'static,
    {
        self.middlewares.push(Arc::new(middleware));
    }

    /// Cria um grupo de rotas aninhado.
    ///
    /// Os middlewares do grupo exterior serão executados antes
    /// dos middlewares do grupo interior.
    pub fn group<F>(&mut self, prefix: &str, configure: F) -> Result<(), String>
    where
        F: FnOnce(&mut RouteGroup) -> Result<(), String>,
    {
        let child_prefix = Self::join_paths(&self.prefix, prefix);

        Router::validate_route(&child_prefix)?;

        let mut child = RouteGroup {
            prefix: child_prefix,
            middlewares: Vec::new(),
            routes: Vec::new(),
        };

        configure(&mut child)?;

        child.apply_middlewares();

        self.routes.extend(child.routes);

        Ok(())
    }

    /// Aplica os middlewares do grupo a todas as rotas registadas.
    ///
    /// Os middlewares do grupo são colocados antes dos middlewares
    /// específicos de cada rota.
    fn apply_middlewares(&mut self) {
        for route in &mut self.routes {
            let mut middlewares = self.middlewares.clone();
            middlewares.append(&mut route.middlewares);
            route.middlewares = middlewares;
        }
    }
    /// Regista uma rota GET.
    pub fn get<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::GET, path, handler)
    }

    /// Regista uma rota POST.
    pub fn post<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::POST, path, handler)
    }

    /// Regista uma rota PUT.
    pub fn put<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.add_fallible(Method::PUT, path, move |request, response| {
            Ok(handler(request, response))
        })
    }

    /// Regista uma rota PATCH.
    pub fn patch<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.add_fallible(Method::PATCH, path, move |request, response| {
            Ok(handler(request, response))
        })
    }

    /// Regista uma rota DELETE.
    pub fn delete<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.add_fallible(Method::DELETE, path, move |request, response| {
            Ok(handler(request, response))
        })
    }

    /// Regista uma rota PUT com handler que pode devolver um erro.
    pub fn put_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::PUT, path, handler)
    }

    /// Regista uma rota PATCH com handler que pode devolver um erro.
    pub fn patch_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::PATCH, path, handler)
    }

    /// Regista uma rota DELETE com handler que pode devolver um erro.
    pub fn delete_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::DELETE, path, handler)
    }
}

/// Representa a aplicação web principal do RustPress.
///
/// A estrutura `App` funciona como ponto de entrada para registrar rotas,
/// middleware e serviços estáticos e, em seguida, iniciar o servidor HTTP.
pub struct App {
    router: Router,
    logger: Logger,
}

impl App {
    /// Regista um grupo de rotas com um prefixo comum.
    ///
    /// As rotas são registadas apenas se a configuração do grupo
    /// terminar com sucesso.
    pub fn group<F>(&mut self, prefix: &str, configure: F) -> Result<(), String>
    where
        F: FnOnce(&mut RouteGroup) -> Result<(), String>,
    {
        let normalized_prefix = if prefix.starts_with('/') {
            prefix.to_owned()
        } else {
            format!("/{prefix}")
        };

        Router::validate_route(&normalized_prefix)?;

        let mut group = RouteGroup {
            prefix: normalized_prefix.trim_end_matches('/').to_owned(),
            middlewares: Vec::new(),
            routes: Vec::new(),
        };

        configure(&mut group)?;

        group.apply_middlewares();

        let routes = group
            .routes
            .into_iter()
            .map(|route| (route.method, route.path, route.handler, route.middlewares))
            .collect();

        self.router.add_group_routes(routes)
    }

    /// Cria uma nova instância da aplicação.
    ///
    /// Normalmente é o primeiro passo antes de registar rotas e middleware.
    pub fn new() -> Self {
        let logger = Logger::new();

        let mut router = Router::new();
        router.set_logger(logger.clone());

        Self { router, logger }
    }

    /// Activa a gestão de sessões HTTP na aplicação.
    ///
    /// As sessões são armazenadas em memória e utilizam a configuração
    /// fornecida para gerir os cookies de sessão.
    /// Activa a gestão de sessões HTTP na aplicação.
    pub fn use_sessions(&mut self, config: SessionConfig) -> Result<(), CookieError> {
        config.validate()?;
        self.router.enable_sessions(config);
        Ok(())
    }
    /// Activa o recebimento de ficheiros através de `multipart/form-data`.
    ///
    /// A configuração é validada antes de ser aplicada à aplicação.
    pub fn use_uploads(&mut self, config: UploadConfig) -> Result<(), UploadError> {
        config.validate()?;
        self.router.enable_uploads(config);
        Ok(())
    }
    /// Define o nível mínimo de registo da aplicação.
    pub fn set_log_level(&mut self, level: Level) {
        self.logger.set_level(level);
    }

    /// Define o handler global para erros das rotas.
    pub fn set_error_handler<F>(&mut self, handler: F)
    where
        F: Fn(&crate::HandlerError, &crate::RequestContext) -> Response + Send + Sync + 'static,
    {
        self.router.set_error_handler(handler);
    }

    /// Regista um handler GET que pode devolver um erro.
    pub fn get<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::GET, path, handler)
    }

    /// Regista um handler POST que pode devolver um erro.
    pub fn post<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::POST, path, handler)
    }

    /// Inicia o servidor HTTP na interface e porta indicadas.
    pub async fn listen(self, address: &str) -> Result<(), ServerError> {
        server::listen(address, self.router, self.logger).await
    }

    /// Inicia a aplicação usando um listener TCP previamente criado.
    ///
    /// Este método permite configurar o socket antes de o entregar ao
    /// RustPress. O listener é consumido pela aplicação.
    ///
    /// # Errors
    /// Devolve um erro se o servidor não conseguir aceitar ligações.
    pub async fn serve_listener(
        self,
        listener: tokio::net::TcpListener,
    ) -> Result<(), ServerError> {
        server::serve(listener, self.router, self.logger).await
    }

    /// Regista um handler para pedidos HTTP do tipo PUT.
    pub fn put<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.router.add(Method::PUT, path, handler)
    }

    /// Regista um handler para pedidos HTTP do tipo PATCH.
    pub fn patch<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.router.add(Method::PATCH, path, handler)
    }

    /// Regista um handler para pedidos HTTP do tipo DELETE.
    pub fn delete<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.router.add(Method::DELETE, path, handler)
    }

    /// Regista um handler PUT que pode devolver um erro.
    pub fn put_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::PUT, path, handler)
    }

    /// Regista um handler PATCH que pode devolver um erro.
    pub fn patch_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::PATCH, path, handler)
    }

    /// Regista um handler DELETE que pode devolver um erro.
    pub fn delete_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::DELETE, path, handler)
    }

    /// Adiciona um middleware global que é executado antes do handler da rota.
    ///
    /// Se o middleware devolver `Some(Response)`, a resposta é interrompida e a
    /// rota correspondente não é processada.
    pub fn use_middleware<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> Option<Response> + Send + Sync + 'static,
    {
        self.router.use_middleware(middleware);
    }

    /// Regista um middleware avançado na cadeia global.
    ///
    /// O middleware pode continuar a execução, devolver uma resposta
    /// imediatamente ou encaminhar um erro para o handler global.
    pub fn use_middleware_advanced<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> MiddlewareResult + Send + Sync + 'static,
    {
        self.router.use_middleware_advanced(middleware);
    }

    /// Serve ficheiros locais a partir de uma pasta do sistema de ficheiros.
    ///
    /// O argumento `url_prefix` define o prefixo do URL a partir do qual os
    /// ficheiros ficam acessíveis, enquanto `directory` representa a pasta em
    /// disco que será exposta publicamente.
    pub fn static_files(&mut self, url_prefix: &str, directory: impl Into<PathBuf>) {
        let prefix = url_prefix.trim_end_matches('/').to_owned();
        let directory = directory.into();

        self.use_middleware(move |req| {
            let method = req.method();

            if method != Method::GET && method != Method::HEAD {
                return None;
            }

            let is_head = method == Method::HEAD;
            let path = req.path();

            // Converte o pedido para um caminho relativo à pasta pública.
            let relative = if path == prefix {
                ""
            } else {
                path.strip_prefix(&format!("{prefix}/"))?
            };

            let relative = if relative.is_empty() {
                "index.html"
            } else {
                relative
            };

            // Garante que não são permitidos acessos a caminhos absolutos,
            // a diretórios com `..` ou a utilização de separadores Windows.
            if relative.contains('\\') || relative.to_ascii_lowercase().contains("%5c") {
                return Some(render_error_page(
                    400,
                    "400 - Pedido inválido",
                    "O caminho solicitado contém caracteres inválidos.",
                ));
            }

            let relative_path = PathBuf::from(relative);

            if !relative_path
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
            {
                return Some(render_error_page(
                    400,
                    "400 - Pedido inválido",
                    "O caminho solicitado contém componentes inválidos.",
                ));
            }

            // Resolve a raiz pública para impedir que ficheiros fora do diretório
            // configurado sejam lidos.
            let Ok(root) = fs::canonicalize(&directory) else {
                return Some(render_error_page(
                    500,
                    "500 - Erro interno do servidor",
                    "Não foi possível aceder ao diretório de ficheiros estáticos.",
                ));
            };

            let Ok(file_path) = fs::canonicalize(root.join(relative_path)) else {
                return Some(render_error_page(
                    404,
                    "404 - Página não encontrada",
                    "O ficheiro solicitado não existe.",
                ));
            };

            // Evita que ficheiros simbólicos ou caminhos manipulados apontem para
            // fora do diretório de arquivos estáticos.
            if !file_path.starts_with(&root) || !file_path.is_file() {
                return Some(render_error_page(
                    404,
                    "404 - Página não encontrada",
                    "O ficheiro solicitado não existe.",
                ));
            }

            // Define o tipo MIME consoante a extensão do ficheiro, de forma a
            // servir corretamente HTML, CSS, JavaScript, imagens e outros tipos.
            let mime = match file_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_ascii_lowercase()
                .as_str()
            {
                "html" => "text/html; charset=utf-8",
                "css" => "text/css; charset=utf-8",
                "js" => "text/javascript; charset=utf-8",
                "json" => "application/json",
                "svg" => "image/svg+xml",
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "ico" => "image/x-icon",
                "txt" => "text/plain; charset=utf-8",
                "pdf" => "application/pdf",
                _ => "application/octet-stream",
            };

            if is_head {
                match fs::File::open(&file_path) {
                    Ok(_) => match fs::metadata(&file_path) {
                        Ok(metadata) => Some(
                            Response::new()
                                .header("Content-Type", mime)
                                .content_length(metadata.len()),
                        ),
                        Err(_) => Some(render_error_page(
                            500,
                            "500 - Erro interno do servidor",
                            "Não foi possível ler os metadados do ficheiro solicitado.",
                        )),
                    },
                    Err(_) => Some(render_error_page(
                        500,
                        "500 - Erro interno do servidor",
                        "Não foi possível ler o ficheiro solicitado.",
                    )),
                }
            } else {
                match fs::read(&file_path) {
                    Ok(bytes) => Some(
                        Response::new()
                            .header("Content-Type", mime)
                            .send_bytes(bytes),
                    ),
                    Err(_) => Some(render_error_page(
                        500,
                        "500 - Erro interno do servidor",
                        "Não foi possível ler o ficheiro solicitado.",
                    )),
                }
            }
        });
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

fn render_error_page(code: u16, title: &str, message: &str) -> Response {
    let mut context = TemplateContext::new();

    context.set("titulo", title.into());
    context.set("codigo", code.into());
    context.set("mensagem", message.into());

    Response::error_page(
        code,
        &format!("views/errors/{code}.html"),
        "views/layouts/main.html",
        "views/partials",
        &context,
    )
}
