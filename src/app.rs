//! RustPress application configuration.
//!
//! [`App`] is the facade used to register routes, middleware, static files,
//! error handlers, and start the server.
//!
//! File paths are interpreted relative to the process working directory.

use hyper::http::Method;

use crate::{
    MiddlewareResult, TemplateContext,
    logger::{Level, Logger},
    request::Request,
    response::Response,
    router::{Handler, Middleware, Router},
    server,
    session::{CookieError, SessionConfig, SessionStore},
    upload::{UploadConfig, UploadError},
};

use std::{
    fs,
    future::Future,
    path::{Component, PathBuf},
    sync::Arc,
};

type ServerError = Box<dyn std::error::Error + Send + Sync>;

/// Represents a route that has not yet been registered with the Router.
struct PendingRoute {
    method: Method,
    path: String,
    handler: Handler,
    middlewares: Vec<Middleware>,
}

/// Allows configuring a set of routes with a common prefix.
pub struct RouteGroup {
    prefix: String,
    middlewares: Vec<Middleware>,
    routes: Vec<PendingRoute>,
}

impl RouteGroup {
    /// Combines the group prefix with the route path.
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

    /// Temporarily registers a fallible route in the group.
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

    /// Registers simple middleware in the group.
    ///
    /// The middleware is applied to all routes in the group.
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

    /// Registers advanced middleware in the group.
    ///
    /// Allows continuing execution, responding directly, or returning an error.
    pub fn use_middleware_advanced<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> MiddlewareResult + Send + Sync + 'static,
    {
        self.middlewares.push(Arc::new(middleware));
    }

    /// Creates a nested route group.
    ///
    /// Middlewares from the outer group are executed before
    /// the middlewares from the inner group.
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

    /// Applies the group's middlewares to all registered routes.
    ///
    /// Group middlewares are placed before route-specific middlewares.
    fn apply_middlewares(&mut self) {
        for route in &mut self.routes {
            let mut middlewares = self.middlewares.clone();
            middlewares.append(&mut route.middlewares);
            route.middlewares = middlewares;
        }
    }

    /// Registers a GET route.
    pub fn get<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::GET, path, handler)
    }

    /// Registers a POST route.
    pub fn post<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::POST, path, handler)
    }

    /// Registers a PUT route whose handler may return an error.
    pub fn put<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::PUT, path, handler)
    }

    /// Registers a PATCH route whose handler may return an error.
    pub fn patch<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::PATCH, path, handler)
    }

    /// Registers a DELETE route whose handler may return an error.
    pub fn delete<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.add_fallible(Method::DELETE, path, handler)
    }
}

/// Represents the main RustPress web application.
///
/// The `App` structure serves as the entry point for registering routes,
/// middleware, and static services, and then starting the HTTP server.
pub struct App {
    router: Router,
    logger: Logger,
}

impl App {
    /// Registers a route group with a common prefix.
    ///
    /// Routes are registered only if the group configuration
    /// completes successfully.
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

    /// Creates a new application instance.
    ///
    /// This is normally the first step before registering routes and middleware.
    pub fn new() -> Self {
        let logger = Logger::new();

        let mut router = Router::new();
        router.set_logger(logger.clone());

        Self { router, logger }
    }

    /// Enables HTTP session management for the application.
    ///
    /// Sessions are stored in memory and use the provided configuration
    /// to manage session cookies.
    pub fn use_sessions(&mut self, config: SessionConfig) -> Result<(), CookieError> {
        config.validate()?;
        self.router.enable_sessions(config);
        Ok(())
    }

    /// Enables session management with a custom storage backend.
    ///
    /// Allows using any [`SessionStore`] implementation,
    /// while preserving cookie configuration and session lifetime.
    pub fn use_session_store<S>(
        &mut self,
        config: SessionConfig,
        store: S,
    ) -> Result<(), CookieError>
    where
        S: SessionStore + 'static,
    {
        config.validate()?;
        self.router.enable_sessions_with_store(config, store);
        Ok(())
    }

    /// Enables receiving files through `multipart/form-data`.
    ///
    /// The configuration is validated before being applied to the application.
    pub fn use_uploads(&mut self, config: UploadConfig) -> Result<(), UploadError> {
        config.validate()?;
        self.router.enable_uploads(config);
        Ok(())
    }

    /// Sets the minimum logging level for the application.
    pub fn set_log_level(&mut self, level: Level) {
        self.logger.set_level(level);
    }

    /// Sets the global error handler for route errors.
    pub fn set_error_handler<F>(&mut self, handler: F)
    where
        F: Fn(&crate::HandlerError, &crate::RequestContext) -> Response + Send + Sync + 'static,
    {
        self.router.set_error_handler(handler);
    }

    /// Registers a GET handler that may return an error.
    pub fn get<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::GET, path, handler)
    }

    /// Registers a POST handler that may return an error.
    pub fn post<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::POST, path, handler)
    }

    /// Registers a new WebSocket route.
    ///
    /// The handler is executed asynchronously after the WebSocket
    /// connection has been established.
    pub fn ws<F, Fut>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, crate::WebSocket) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.router.add_websocket(path, handler)
    }

    /// Starts the HTTP server on the specified interface and port.
    pub async fn listen(self, address: &str) -> Result<(), ServerError> {
        server::listen(address, self.router, self.logger).await
    }

    /// Starts the application using a previously created TCP listener.
    ///
    /// This method allows configuring the socket before passing it
    /// to RustPress. The listener is consumed by the application.
    ///
    /// # Errors
    /// Returns an error if the server cannot accept connections.
    pub async fn serve_listener(
        self,
        listener: tokio::net::TcpListener,
    ) -> Result<(), ServerError> {
        server::serve(listener, self.router, self.logger).await
    }

    /// Registers a PUT handler that may return an error.
    pub fn put<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::PUT, path, handler)
    }

    /// Registers a PATCH handler that may return an error.
    pub fn patch<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::PATCH, path, handler)
    }

    /// Registers a DELETE handler that may return an error.
    pub fn delete<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::DELETE, path, handler)
    }

    /// Adds global middleware that runs before the route handler.
    ///
    /// If the middleware returns `Some(Response)`, the response is sent
    /// immediately and the corresponding route is not processed.
    pub fn use_middleware<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> Option<Response> + Send + Sync + 'static,
    {
        self.router.use_middleware(middleware);
    }

    /// Registers advanced middleware in the global chain.
    ///
    /// The middleware can continue execution, return a response
    /// immediately, or forward an error to the global handler.
    pub fn use_middleware_advanced<F>(&mut self, middleware: F)
    where
        F: Fn(&Request) -> MiddlewareResult + Send + Sync + 'static,
    {
        self.router.use_middleware_advanced(middleware);
    }

    /// Serves local files from a directory in the file system.
    ///
    /// The `url_prefix` argument defines the URL prefix from which the
    /// files become accessible, while `directory` represents the directory
    /// on disk that will be exposed publicly.
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

            // Converts the request into a path relative to the public directory.
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

            // Prevents access to absolute paths, directories containing `..`,
            // or the use of Windows path separators.
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

            // Resolves the public root to prevent files outside the
            // configured directory from being read.
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

            // Prevents symbolic links or manipulated paths from pointing
            // outside the static files directory.
            if !file_path.starts_with(&root) || !file_path.is_file() {
                return Some(render_error_page(
                    404,
                    "404 - Página não encontrada",
                    "O ficheiro solicitado não existe.",
                ));
            }

            // Determines the MIME type based on the file extension in order
            // to correctly serve HTML, CSS, JavaScript, images, and other types.
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
