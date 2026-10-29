use rustpress::{App, MiddlewareResult, Request, Response};
use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;

fn request_with_app(app: App, method: &str, path: &str) -> String {
    let listener =
        std::net::TcpListener::bind("127.0.0.1:0").expect("Não foi possível reservar uma porta");

    listener.set_nonblocking(true).unwrap();

    let address = listener.local_addr().unwrap();

    let _server_thread = thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().unwrap();

        runtime.block_on(async move {
            let listener = TcpListener::from_std(listener).unwrap();
            app.serve_listener(listener).await.unwrap();
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

            stream.write_all(request.as_bytes()).unwrap();

            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();

            return response;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
fn advanced_middleware_next_allows_handler_to_continue() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| MiddlewareResult::Next);

    app.get("/teste-middleware", |_request, response| {
        Ok(response.send("Handler executado"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/teste-middleware");

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Handler executado"),
        "O handler da rota não foi executado: {response}"
    );
}

#[test]
fn advanced_middleware_respond_stops_handler_execution() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Respond(Response::new().send("Resposta do middleware"))
    });

    app.get("/resposta-middleware", |_request, response| {
        Ok(response.send("Este handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/resposta-middleware");

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Resposta do middleware"),
        "A resposta do middleware não foi encontrada: {response}"
    );

    assert!(
        !response.contains("Este handler não deveria executar"),
        "O handler da rota foi executado indevidamente: {response}"
    );
}

#[test]
fn advanced_middleware_error_uses_global_error_handler() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Error(rustpress::HandlerError::new("Erro interno confidencial"))
    });

    app.get("/erro-middleware", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Erro do middleware em {} {}",
            context.method(),
            context.path()
        ))
    });

    let response = request_with_app(app, "GET", "/erro-middleware");

    assert!(
        response.contains("500 Internal Server Error"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Erro do middleware em GET /erro-middleware"),
        "O handler global não tratou o erro: {response}"
    );

    assert!(
        !response.contains("Erro interno confidencial"),
        "A resposta revelou detalhes internos: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler da rota foi executado indevidamente: {response}"
    );
}

#[test]
fn advanced_middleware_error_uses_default_error_response() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Error(rustpress::HandlerError::new("Erro interno confidencial"))
    });

    app.get("/erro-padrao", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/erro-padrao");

    assert!(
        response.contains("500 Internal Server Error"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        !response.contains("Erro interno confidencial"),
        "A resposta revelou detalhes internos: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler da rota foi executado indevidamente: {response}"
    );
}

#[test]
fn advanced_middleware_respond_stops_middleware_chain() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Respond(Response::new().send("Resposta do primeiro middleware"))
    });

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Respond(Response::new().send("O segundo middleware não deveria executar"))
    });

    app.get("/cadeia", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/cadeia");

    assert!(
        response.contains("Resposta do primeiro middleware"),
        "A primeira resposta não foi encontrada: {response}"
    );

    assert!(
        !response.contains("O segundo middleware não deveria executar"),
        "O segundo middleware foi executado indevidamente: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler da rota foi executado indevidamente: {response}"
    );
}

#[test]
fn advanced_middleware_error_stops_middleware_chain() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Error(rustpress::HandlerError::new("Erro interno confidencial"))
    });

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Respond(Response::new().send("O segundo middleware não deveria executar"))
    });

    app.get("/erro-cadeia", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/erro-cadeia");

    assert!(
        response.contains("500 Internal Server Error"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        !response.contains("O segundo middleware não deveria executar"),
        "O segundo middleware foi executado indevidamente: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler da rota foi executado indevidamente: {response}"
    );

    assert!(
        !response.contains("Erro interno confidencial"),
        "A resposta revelou detalhes internos: {response}"
    );
}
