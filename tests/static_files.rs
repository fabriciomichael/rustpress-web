use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};

use rustpress::{App, Request, Response};
use tokio::net::TcpListener;

fn request_static_file(path: &str) -> String {
    request_static_file_with_method("GET", path)
}

fn request_static_file_with_method(method: &str, path: &str) -> String {
    // Mantém aqui o restante código actual de request_static_file(),
    // desde a criação do listener até à leitura da resposta.
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("Não foi possível reservar uma porta livre");

    listener
        .set_nonblocking(true)
        .expect("Não foi possível configurar o listener");

    let address = listener
        .local_addr()
        .expect("Não foi possível obter o endereço");

    let mut app = App::new();
    app.static_files("/assets", "public");

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

fn request_dynamic_route_with_method(method: &str, path: &str) -> String {
    let listener = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("Não foi possível reservar uma porta livre");

    listener
        .set_nonblocking(true)
        .expect("Não foi possível configurar o listener");

    let address = listener
        .local_addr()
        .expect("Não foi possível obter o endereço");

    let mut app = App::new();

    app.get("/dinamico", |_req: Request, res: Response| {
        res.header("Content-Type", "text/plain; charset=utf-8")
            .send("Conteúdo dinâmico para GET e HEAD")
    })
    .expect("Não foi possível registar a rota dinâmica");

    app.post("/dinamico", |_req: Request, res: Response| {
        res.send("Recurso criado")
    })
    .expect("Não foi possível registar a rota POST");

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
    assert!(
        response.contains("404 - Página não encontrada"),
        "{response}"
    );
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
        !response.contains("name = \"rustpress\""),
        "O conteúdo do Cargo.toml não deve ser exposto:\n{response}"
    );
}

#[test]
fn head_returns_headers_without_file_body() {
    let expected_length = std::fs::metadata("public/teste.txt")
        .expect("Não foi possível obter os metadados de teste.txt")
        .len();
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
