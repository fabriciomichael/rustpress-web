use rustpress::{App, MiddlewareResult, Request, Response, session::SessionConfig};
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

fn request_twice_with_cookie(app: App, method: &str, path: &str) -> (String, String) {
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

    fn send_request(
        address: &std::net::SocketAddr,
        method: &str,
        path: &str,
        cookie: Option<&str>,
    ) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            if Instant::now() >= deadline {
                panic!("O servidor não ficou disponível a tempo");
            }

            if let Ok(mut stream) = TcpStream::connect_timeout(address, Duration::from_millis(200))
            {
                let mut request =
                    format!("{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n");

                if let Some(cookie) = cookie {
                    request.push_str(&format!("Cookie: {cookie}\r\n"));
                }

                request.push_str("\r\n");

                stream.write_all(request.as_bytes()).unwrap();

                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();

                return response;
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    let first_response = send_request(&address, method, path, None);

    let cookie = first_response
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim())
        .and_then(|value| value.split(';').next())
        .expect("A primeira resposta não enviou o cookie da sessão");

    let second_response = send_request(&address, method, path, Some(cookie));

    (first_response, second_response)
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

#[test]
fn session_persists_data_between_requests() {
    let mut app = App::new();

    app.use_sessions(SessionConfig::default());

    app.get("/contador", |request, response| {
        let session = request.session().expect("A sessão não foi disponibilizada");

        let mut session = session.lock().unwrap();

        let count = session.get::<u32>("count").unwrap_or(0) + 1;

        session.insert("count", count).unwrap();

        Ok(response.send(count.to_string()))
    })
    .expect("Não foi possível registar a rota");

    let (first_response, second_response) = request_twice_with_cookie(app, "GET", "/contador");

    assert!(
        first_response.contains("200 OK"),
        "Primeira resposta inesperada: {first_response}"
    );

    assert!(
        first_response.ends_with("\r\n\r\n1"),
        "O primeiro contador deveria ser 1: {first_response}"
    );

    assert!(
        second_response.contains("200 OK"),
        "Segunda resposta inesperada: {second_response}"
    );

    assert!(
        second_response.ends_with("\r\n\r\n2"),
        "O segundo contador deveria ser 2: {second_response}"
    );
}

#[test]
fn regenerates_session_id_and_sends_new_cookie() {
    let mut app = App::new();
    app.use_sessions(SessionConfig::default());

    app.get("/session", |request, response| {
        let session_handle = request.session().expect("A sessão não foi disponibilizada");

        let username = {
            let mut session = session_handle.lock().unwrap();

            match session.get::<String>("username") {
                Some(username) => {
                    let id = session.id().to_string();
                    return Ok(response.send(format!("{id}:{username}")));
                }
                None => {
                    session.insert("username", "fabricio").unwrap();
                    session.id().to_string()
                }
            }
        };

        let old_id = username;

        assert!(request.regenerate_session());

        let new_id = session_handle.lock().unwrap().id().to_string();

        Ok(response.send(format!("{old_id}:{new_id}")))
    })
    .expect("Não foi possível registar a rota");

    let (first_response, second_response) = request_twice_with_cookie(app, "GET", "/session");

    let first_body = first_response.split("\r\n\r\n").nth(1).unwrap();

    let second_body = second_response.split("\r\n\r\n").nth(1).unwrap();

    let (old_id, new_id) = first_body.split_once(':').unwrap();

    assert_ne!(old_id, new_id);
    assert!(
        first_response
            .to_ascii_lowercase()
            .contains(&format!("set-cookie: rustpress_session={new_id}").to_ascii_lowercase())
    );
    assert_eq!(second_body, format!("{new_id}:fabricio"));
}
