use hyper::http::Method;

use crate::{TemplateContext, request::Request, response::Response, router::Router, server};

use std::{
    fs,
    path::{Component, PathBuf},
};

type ServerError = Box<dyn std::error::Error + Send + Sync>;

/// Representa a aplicação web principal do RustPress.
///
/// A estrutura `App` funciona como ponto de entrada para registrar rotas,
/// middleware e serviços estáticos e, em seguida, iniciar o servidor HTTP.
pub struct App {
    router: Router,
}

impl App {
    /// Cria uma nova instância da aplicação.
    ///
    /// Normalmente é o primeiro passo antes de registar rotas e middleware.
    pub fn new() -> Self {
        Self {
            router: Router::new(),
        }
    }

    /// Regista um handler para pedidos HTTP do tipo GET.
    pub fn get<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.router.add(Method::GET, path, handler)
    }

    /// Regista um handler para pedidos HTTP do tipo POST.
    pub fn post<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Response + Send + Sync + 'static,
    {
        self.router.add(Method::POST, path, handler)
    }

    /// Inicia o servidor HTTP na interface e porta indicadas.
    pub async fn listen(self, address: &str) -> Result<(), ServerError> {
        server::listen(address, self.router).await
    }

    pub async fn serve_listener(
        self,
        listener: tokio::net::TcpListener,
    ) -> Result<(), ServerError> {
        server::serve(listener, self.router).await
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
            } else if let Some(rest) = path.strip_prefix(&format!("{prefix}/")) {
                rest
            } else {
                return None;
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
                                .header("Content-Length", &metadata.len().to_string()),
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

    /// Regista um handler GET que pode devolver um erro.
    pub fn get_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::GET, path, handler)
    }

    /// Regista um handler POST que pode devolver um erro.
    pub fn post_fallible<F>(&mut self, path: &str, handler: F) -> Result<(), String>
    where
        F: Fn(Request, Response) -> Result<Response, crate::HandlerError> + Send + Sync + 'static,
    {
        self.router.add_fallible(Method::POST, path, handler)
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
