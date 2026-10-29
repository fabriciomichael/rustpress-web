use std::{
    fs,
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use rustpress::{App, HandlerError, Request, Response};
use tokio::net::TcpListener;

fn request_with_app(app: App, method: &str, path: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("Não foi possível reservar uma porta livre");

    listener
        .set_nonblocking(true)
        .expect("Não foi possível configurar o listener");

    let address = listener
        .local_addr()
        .expect("Não foi possível obter o endereço");

    let _server_thread = thread::spawn(move || {
        let runtime =
            tokio::runtime::Runtime::new().expect("Não foi possível criar o runtime Tokio");

        runtime.block_on(async move {
            let listener =
                TcpListener::from_std(listener).expect("Não foi possível converter o listener");

            app.serve_listener(listener)
                .await
                .expect("Erro no servidor de teste");
        });
    });

    let deadline = Instant::now() + Duration::from_secs(10);

    loop {
        if Instant::now() >= deadline {
            panic!("O servidor não ficou disponível a tempo");
        }

        if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200)) {
            let request =
                format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n");

            stream
                .write_all(request.as_bytes())
                .expect("Falha ao enviar pedido HTTP");

            let mut response = String::new();

            stream
                .read_to_string(&mut response)
                .expect("Falha ao ler resposta HTTP");

            return response;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn request_static_file(path: &str) -> String {
    request_static_file_with_method("GET", path)
}

fn request_static_file_with_method(method: &str, path: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("O relógio do sistema está incorrecto")
        .as_nanos();

    let directory = std::env::temp_dir().join(format!("rustpress-static-files-{unique}"));

    fs::create_dir_all(directory.join("css")).expect("Não foi possível criar a pasta temporária");

    fs::write(
        directory.join("teste.txt"),
        "RustPress: ficheiro estatico acessivel.",
    )
    .expect("Não foi possível criar o ficheiro de texto");

    fs::write(
        directory.join("css").join("style.css"),
        "body { margin: 0; }",
    )
    .expect("Não foi possível criar o ficheiro CSS");

    fs::write(
        directory.join("teste.js"),
        "console.log(\"RustPress está a servir JavaScript!\");",
    )
    .expect("Não foi possível criar o ficheiro JavaScript");

    let mut app = App::new();
    app.static_files("/assets", &directory);

    let response = request_with_app(app, method, path);

    fs::remove_dir_all(&directory).expect("Não foi possível remover a pasta temporária");

    response
}

fn request_dynamic_route_with_method(method: &str, path: &str) -> String {
    let mut app = App::new();

    app.get("/dinamico", |_req: Request, res: Response| {
        Ok(res
            .header("Content-Type", "text/plain; charset=utf-8")
            .send("Conteúdo dinâmico para GET e HEAD"))
    })
    .expect("Não foi possível registar a rota dinâmica");

    app.post("/dinamico", |_req: Request, res: Response| {
        Ok(res.send("Recurso criado"))
    })
    .expect("Não foi possível registar a rota POST");

    request_with_app(app, method, path)
}

fn request_middleware_response() -> String {
    let mut app = App::new();

    app.use_middleware(|_| None);

    app.get("/middleware-teste", |_req: Request, res: Response| {
        Ok(res.send("Handler executado"))
    })
    .expect("Não foi possível registar a rota de teste");

    request_with_app(app, "GET", "/middleware-teste")
}

fn request_middleware_short_circuit() -> String {
    let mut app = App::new();

    app.use_middleware(|_| Some(Response::new().status(401).send("Acesso negado")));

    app.get("/middleware-teste", |_req: Request, res: Response| {
        Ok(res.send("Handler executado"))
    })
    .expect("Não foi possível registar a rota de teste");

    request_with_app(app, "GET", "/middleware-teste")
}

fn request_fallible_route_with_method(method: &str) -> String {
    let mut app = App::new();

    match method {
        "PUT" => app
            .put_fallible("/teste", |_req, _res| {
                Err(std::io::Error::other("Erro de teste").into())
            })
            .expect("Erro ao registar PUT"),

        "PATCH" => app
            .patch_fallible("/teste", |_req, _res| {
                Err(std::io::Error::other("Erro de teste").into())
            })
            .expect("Erro ao registar PATCH"),

        "DELETE" => app
            .delete_fallible("/teste", |_req, _res| {
                Err(std::io::Error::other("Erro de teste").into())
            })
            .expect("Erro ao registar DELETE"),

        _ => panic!("Método não suportado neste teste: {method}"),
    }

    request_with_app(app, method, "/teste")
}

#[test]
fn serves_existing_static_file() {
    let response = request_static_file("/assets/teste.txt");

    assert!(response.contains("200 OK"), "{response}");
    assert!(
        response.contains("RustPress: ficheiro estatico acessivel."),
        "{response}"
    );
}

#[test]
fn returns_404_for_missing_static_file() {
    let response = request_static_file("/assets/ficheiro-inexistente.txt");

    assert!(response.contains("404 Not Found"), "{response}");
    assert!(response.contains("Página não encontrada"), "{response}");
    assert!(response.contains("Voltar à página inicial"), "{response}");
    assert!(response.contains("Desenvolvido com RustPress"), "{response}");
}

#[test]
fn serves_css_with_correct_content_type() {
    let response = request_static_file("/assets/css/style.css");

    assert!(response.contains("200 OK"), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/css; charset=utf-8"),
        "{response}"
    );
}

#[test]
fn serves_javascript_with_correct_content_type() {
    let response = request_static_file("/assets/teste.js");

    assert!(response.contains("200 OK"), "{response}");

    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/javascript; charset=utf-8"),
        "{response}"
    );

    assert!(
        response.contains("console.log(\"RustPress está a servir JavaScript!\");"),
        "{response}"
    );
}

#[test]
fn blocks_path_traversal_to_project_files() {
    let response = request_static_file("/assets/../Cargo.toml");

    assert!(
        response.contains("400 Bad Request"),
        "Era esperada uma resposta 400 Bad Request:\n{response}"
    );

    assert!(
        !response.contains("name = \"rustpress\""),
        "O conteúdo do Cargo.toml não deve ser exposto:\n{response}"
    );
}

#[test]
fn blocks_encoded_path_traversal() {
    let response = request_static_file("/assets/%2e%2e/Cargo.toml");

    assert!(
        !response.contains("name = \"rustpress\""),
        "O Cargo.toml não deve ser exposto através de um caminho codificado:\n{response}"
    );
}

#[test]
fn blocks_encoded_backslash() {
    let response = request_static_file("/assets/%5cCargo.toml");

    assert!(
        response.contains("400 Bad Request"),
        "Era esperada uma resposta 400 Bad Request:\n{response}"
    );

    assert!(
        !response.contains("name = \"rustpress\""),
        "O conteúdo do Cargo.toml não deve ser exposto:\n{response}"
    );
}

#[test]
fn returns_404_for_missing_nested_static_file() {
    let response = request_static_file("/assets/pasta-inexistente/segredo.txt");

    assert!(
        response.contains("404 Not Found"),
        "Era esperada uma resposta 404 Not Found:\n{response}"
    );

    assert!(
        response.contains("Página não encontrada"),
        "Era esperada a página personalizada 404:\n{response}"
    );

    assert!(
        !response.contains("name = \"rustpress\""),
        "A resposta não deve expor informações internas:\n{response}"
    );
}

#[test]
fn head_returns_headers_without_file_body() {
    let expected_length = "RustPress: ficheiro estatico acessivel.".len() as u64;
    let response = request_static_file_with_method("HEAD", "/assets/teste.txt");

    assert!(
        response.contains("200 OK"),
        "Era esperada uma resposta 200 OK:\n{response}"
    );

    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/plain; charset=utf-8"),
        "O Content-Type está incorrecto:\n{response}"
    );

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("");

    assert!(
        body.is_empty(),
        "Uma resposta HEAD não deve conter o corpo do ficheiro:\n{response}"
    );

    let expected_header = format!("content-length: {expected_length}");

    assert!(
        response.to_ascii_lowercase().contains(&expected_header),
        "O Content-Length deveria corresponder a {expected_length} bytes:\n{response}"
    );
}

#[test]
fn head_returns_headers_without_dynamic_route_body() {
    let expected_body = "Conteúdo dinâmico para GET e HEAD";
    let expected_length = expected_body.len();

    let response = request_dynamic_route_with_method("HEAD", "/dinamico");

    assert!(
        response.contains("200 OK"),
        "Era esperada uma resposta 200 OK:\n{response}"
    );

    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-type: text/plain; charset=utf-8"),
        "O Content-Type está incorrecto:\n{response}"
    );

    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body)
        .unwrap_or("");

    assert!(
        body.is_empty(),
        "Uma resposta HEAD não deve conter o corpo da rota dinâmica:\n{response}"
    );

    let expected_header = format!("content-length: {expected_length}");

    assert!(
        response.to_ascii_lowercase().contains(&expected_header),
        "O Content-Length deveria corresponder a {expected_length} bytes:\n{response}"
    );
}

#[test]
fn returns_405_with_allow_header_for_unsupported_method() {
    let response = request_dynamic_route_with_method("PUT", "/dinamico");

    assert!(
        response.contains("405 Method Not Allowed"),
        "Era esperada uma resposta 405 Method Not Allowed:\n{response}"
    );

    let headers = response
        .split_once("\r\n\r\n")
        .map(|(headers, _)| headers)
        .unwrap_or("")
        .to_ascii_lowercase();

    assert!(
        headers.contains("allow: get, head, post"),
        "O cabeçalho Allow deveria indicar GET, HEAD e POST:\n{response}"
    );
}

#[test]
fn returns_404_for_unknown_dynamic_route() {
    let response = request_dynamic_route_with_method("GET", "/rota-inexistente");

    assert!(
        response.contains("404 Not Found"),
        "Era esperada uma resposta 404 Not Found:\n{response}"
    );

    assert!(
        !response.contains("405 Method Not Allowed"),
        "Um caminho inexistente não deve devolver 405:\n{response}"
    );
}

#[test]
fn put_fallible_returns_500_without_leaking_error_details() {
    let response = request_fallible_route_with_method("PUT");

    assert!(response.contains("500 Internal Server Error"), "{response}");

    assert!(
        !response.contains("Erro de teste"),
        "A resposta revelou detalhes internos: {response}"
    );
}

#[test]
fn patch_fallible_returns_500_when_handler_fails() {
    let response = request_fallible_route_with_method("PATCH");
    assert!(response.contains("500 Internal Server Error"), "{response}");
}

#[test]
fn delete_fallible_returns_500_when_handler_fails() {
    let response = request_fallible_route_with_method("DELETE");
    assert!(response.contains("500 Internal Server Error"), "{response}");
}

#[test]
fn middleware_none_allows_handler_to_continue() {
    let response = request_middleware_response();

    assert!(
        response.contains("200 OK"),
        "Esperava uma resposta HTTP 200, mas recebi:\n{response}"
    );

    assert!(
        response.contains("Handler executado"),
        "O middleware impediu inesperadamente a execução do handler:\n{response}"
    );
}

#[test]
fn middleware_response_stops_handler_execution() {
    let response = request_middleware_short_circuit();

    assert!(
        response.contains("401 Unauthorized"),
        "Esperava HTTP 401:\n{response}"
    );

    assert!(
        response.contains("Acesso negado"),
        "A resposta do middleware não foi devolvida:\n{response}"
    );

    assert!(
        !response.contains("Handler executado"),
        "O handler foi executado apesar da interrupção:\n{response}"
    );
}

#[test]
fn fallible_handler_returns_custom_status_without_leaking_error() {
    let mut app = App::new();

    app.get("/erro-personalizado", |_req, _res| {
        Err(HandlerError::with_status(
            404,
            "detalhes internos confidenciais",
        ))
    })
    .expect("Erro ao registar a rota");

    let response = request_with_app(app, "GET", "/erro-personalizado");

    assert!(response.contains("404 Not Found"), "{response}");
    assert!(
        !response.contains("detalhes internos confidenciais"),
        "A resposta revelou detalhes internos: {response}"
    );
}

#[test]
fn global_error_handler_customizes_response_using_request_context() {
    let mut app = App::new();

    app.get("/erro-global", |_req, _res| {
        Err(HandlerError::new("detalhes internos confidenciais"))
    })
    .expect("Erro ao registar a rota");

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Erro ao processar {} {}",
            context.method(),
            context.path()
        ))
    });

    let response = request_with_app(app, "GET", "/erro-global");

    assert!(
        response.contains("500 Internal Server Error"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Erro ao processar GET /erro-global"),
        "A resposta personalizada não foi encontrada: {response}"
    );

    assert!(
        !response.contains("detalhes internos confidenciais"),
        "A resposta revelou detalhes internos: {response}"
    );
}

#[test]
fn global_error_handler_customizes_not_found_response() {
    let mut app = App::new();

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Erro HTTP {} no caminho {}",
            error.status(),
            context.path()
        ))
    });

    let response = request_with_app(app, "GET", "/rota-inexistente");

    assert!(
        response.contains("404 Not Found"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Erro HTTP 404 no caminho /rota-inexistente"),
        "A resposta personalizada não foi encontrada: {response}"
    );
}

#[test]
fn global_error_handler_customizes_method_not_allowed_response() {
    let mut app = App::new();

    app.get("/recurso", |_req, _res| {
        Ok(Response::new().send("Recurso disponível"))
    })
    .expect("Erro ao registar a rota");

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Método {} não permitido em {}",
            context.method(),
            context.path()
        ))
    });

    let response = request_with_app(app, "POST", "/recurso");

    assert!(
        response.contains("405 Method Not Allowed"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Método POST não permitido em /recurso"),
        "A resposta personalizada não foi encontrada: {response}"
    );

    assert!(
        response.to_ascii_lowercase().contains("allow: get"),
        "O cabeçalho Allow não foi preservado: {response}"
    );
}
