use rustpress_web::{
    App, AuthMiddleware, MiddlewareResult, Request, Response, RoleMiddleware,
    session::SessionConfig,
};
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

fn request_with_body(app: App, method: &str, path: &str, body: &[u8]) -> String {
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
            let headers = format!(
                "{method} {path} HTTP/1.1\r\n\
                 Host: {address}\r\n\
                 Content-Length: {}\r\n\
                 Connection: close\r\n\r\n",
                body.len()
            );

            stream.write_all(headers.as_bytes()).unwrap();
            stream.write_all(body).unwrap();

            let mut response = String::new();
            stream.read_to_string(&mut response).unwrap();

            return response;
        }

        thread::sleep(Duration::from_millis(50));
    }
}

fn request_with_headers(app: App, method: &str, path: &str, headers: &[(&str, &str)]) -> String {
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
            let mut request = format!(
                "{method} {path} HTTP/1.1\r\n\
                 Host: {address}\r\n\
                 Connection: close\r\n"
            );

            for (name, value) in headers {
                request.push_str(&format!("{name}: {value}\r\n"));
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

fn request_twice_with_app(
    app: App,
    first_method: &str,
    first_path: &str,
    second_method: &str,
    second_path: &str,
) -> (String, String) {
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

    fn send_request(address: &std::net::SocketAddr, method: &str, path: &str) -> String {
        let deadline = Instant::now() + Duration::from_secs(10);

        loop {
            if Instant::now() >= deadline {
                panic!("O servidor não ficou disponível a tempo");
            }

            if let Ok(mut stream) = TcpStream::connect_timeout(address, Duration::from_millis(200))
            {
                let request = format!(
                    "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
                );

                stream.write_all(request.as_bytes()).unwrap();

                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();

                return response;
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    let first_response = send_request(&address, first_method, first_path);

    let second_response = send_request(&address, second_method, second_path);

    (first_response, second_response)
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

fn request_then_with_cookie(
    app: App,
    first_method: &str,
    first_path: &str,
    second_method: &str,
    second_path: &str,
) -> (String, String) {
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
                let mut request = format!(
                    "{method} {path} HTTP/1.1\r\n\
                     Host: {address}\r\n\
                     Connection: close\r\n"
                );

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

    let first_response = send_request(&address, first_method, first_path, None);

    let cookie = first_response
        .lines()
        .find(|line| line.to_ascii_lowercase().starts_with("set-cookie:"))
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value.trim())
        .and_then(|value| value.split(';').next())
        .expect("A primeira resposta não enviou o cookie da sessão");

    let second_response = send_request(&address, second_method, second_path, Some(cookie));

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
        MiddlewareResult::Error(rustpress_web::HandlerError::new(
            "Erro interno confidencial",
        ))
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
        MiddlewareResult::Error(rustpress_web::HandlerError::new(
            "Erro interno confidencial",
        ))
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
        MiddlewareResult::Error(rustpress_web::HandlerError::new(
            "Erro interno confidencial",
        ))
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

    app.use_sessions(SessionConfig::default()).unwrap();

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
    app.use_sessions(SessionConfig::default()).unwrap();

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

        assert!(request.regenerate_session().unwrap());

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

#[test]
fn route_handler_panic_uses_global_error_handler_and_server_continues() {
    let mut app = App::new();

    app.get("/panic", |_request, _response| {
        panic!("Erro simulado no handler");
    })
    .expect("Não foi possível registar a rota");

    app.get("/normal", |_request, response| {
        Ok(response.send("Servidor continua funcional"))
    })
    .expect("Não foi possível registar a rota");

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Erro global em {} {}",
            context.method(),
            context.path()
        ))
    });

    let (panic_response, normal_response) =
        request_twice_with_app(app, "GET", "/panic", "GET", "/normal");

    assert!(
        panic_response.contains("500 Internal Server Error"),
        "O panic deveria produzir HTTP 500: {panic_response}"
    );

    assert!(
        panic_response.contains("Erro global em GET /panic"),
        "O Global Error Handler não tratou o panic: {panic_response}"
    );

    assert!(
        !panic_response.contains("Erro simulado no handler"),
        "A resposta revelou a mensagem técnica do panic: {panic_response}"
    );

    assert!(
        normal_response.contains("200 OK"),
        "O servidor não continuou a responder: {normal_response}"
    );

    assert!(
        normal_response.contains("Servidor continua funcional"),
        "A segunda rota não foi executada: {normal_response}"
    );
}

#[test]
fn middleware_panic_uses_global_error_handler_and_server_continues() {
    let mut app = App::new();

    app.use_middleware_advanced(|request: &Request| {
        if request.path() == "/middleware-panic" {
            panic!("Erro simulado no middleware");
        }

        MiddlewareResult::Next
    });

    app.get("/normal", |_request, response| {
        Ok(response.send("Servidor continua funcional"))
    })
    .expect("Não foi possível registar a rota");

    app.set_error_handler(|error, context| {
        Response::new().status(error.status()).send(format!(
            "Erro global em {} {}",
            context.method(),
            context.path()
        ))
    });

    let (panic_response, normal_response) =
        request_twice_with_app(app, "GET", "/middleware-panic", "GET", "/normal");

    assert!(
        panic_response.contains("500 Internal Server Error"),
        "O panic deveria produzir HTTP 500: {panic_response}"
    );

    assert!(
        panic_response.contains("Erro global em GET /middleware-panic"),
        "O Global Error Handler não tratou o panic: {panic_response}"
    );

    assert!(
        !panic_response.contains("Erro simulado no middleware"),
        "A resposta revelou detalhes técnicos: {panic_response}"
    );

    assert!(
        normal_response.contains("200 OK"),
        "O servidor não continuou a responder: {normal_response}"
    );

    assert!(
        normal_response.contains("Servidor continua funcional"),
        "A segunda rota não foi executada: {normal_response}"
    );
}

#[test]
fn request_body_accepts_normal_body() {
    let mut app = App::new();

    app.post("/body", |_request, response| {
        Ok(response.send("Pedido recebido"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(app, "POST", "/body", b"Dados de teste");

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Pedido recebido"),
        "O handler não recebeu o pedido: {response}"
    );
}

#[test]
fn request_body_rejects_body_larger_than_limit() {
    let mut app = App::new();

    app.post("/body-grande", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let body = vec![b'A'; 1024 * 1024 + 1];

    let response = request_with_body(app, "POST", "/body-grande", &body);

    assert!(
        response.contains("413"),
        "Era esperada uma resposta HTTP 413: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler executou apesar do corpo exceder o limite"
    );
}

#[test]
fn middleware_can_stop_request_before_body_is_read() {
    let mut app = App::new();

    app.use_middleware_advanced(|_request: &Request| {
        MiddlewareResult::Respond(Response::new().send("Pedido bloqueado pelo middleware"))
    });

    app.post("/body-bloqueado", |_request, response| {
        Ok(response.send("O handler não deveria executar"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(
        app,
        "POST",
        "/body-bloqueado",
        b"Dados que nao devem ser processados",
    );

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Pedido bloqueado pelo middleware"),
        "O middleware não interrompeu o pedido: {response}"
    );

    assert!(
        !response.contains("O handler não deveria executar"),
        "O handler executou indevidamente"
    );
}

#[test]
fn request_body_can_be_read_as_bytes() {
    let mut app = App::new();

    app.post("/body-bytes", |request, response| {
        let body = request.body();

        assert_eq!(body.as_ref(), b"Dados de teste");

        Ok(response.send("Bytes recebidos"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(app, "POST", "/body-bytes", b"Dados de teste");

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(response.contains("Bytes recebidos"));
}

#[test]
fn request_body_can_be_read_as_text() {
    let mut app = App::new();

    app.post("/body-texto", |request, response| {
        let body = request
            .text()
            .map_err(|_| rustpress_web::HandlerError::new("Texto inválido"))?;

        Ok(response.send(body))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(app, "POST", "/body-texto", "Olá, RustPress!".as_bytes());

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(response.contains("Olá, RustPress!"));
}

#[test]
fn request_body_rejects_invalid_utf8_text() {
    let mut app = App::new();

    app.post("/body-utf8", |request, response| {
        let body = request
            .text()
            .map_err(|_| rustpress_web::HandlerError::new("Texto inválido"))?;

        Ok(response.send(body))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(app, "POST", "/body-utf8", &[0xFF, 0xFE, 0xFD]);

    assert!(
        response.contains("500 Internal Server Error"),
        "Era esperada uma resposta de erro: {response}"
    );

    assert!(
        !response.contains("Texto inválido"),
        "A resposta revelou a mensagem interna: {response}"
    );
}

#[test]
fn request_body_can_be_deserialized_from_json() {
    #[derive(serde::Deserialize)]
    struct NovoUtilizador {
        nome: String,
        idade: u8,
    }

    let mut app = App::new();

    app.post("/utilizadores", |request, response| {
        let utilizador = request
            .json::<NovoUtilizador>()
            .map_err(|_| rustpress_web::HandlerError::with_status(400, "JSON inválido"))?;

        Ok(response.send(format!(
            "Nome: {}, Idade: {}",
            utilizador.nome, utilizador.idade
        )))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(
        app,
        "POST",
        "/utilizadores",
        r#"{"nome":"Fabrício","idade":25}"#.as_bytes(),
    );

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(
        response.contains("Nome: Fabrício, Idade: 25"),
        "Os dados JSON não foram processados correctamente: {response}"
    );
}

#[test]
fn request_body_rejects_invalid_json() {
    #[derive(serde::Deserialize)]
    struct NovoUtilizador {
        nome: String,
    }

    let mut app = App::new();

    app.post("/utilizadores-invalidos", |request, response| {
        let utilizador = request
            .json::<NovoUtilizador>()
            .map_err(|_| rustpress_web::HandlerError::with_status(400, "JSON inválido"))?;

        Ok(response.send(utilizador.nome))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(app, "POST", "/utilizadores-invalidos", br#"{"nome":}"#);

    assert!(
        response.contains("400"),
        "Era esperada uma resposta HTTP 400: {response}"
    );
}

#[test]
fn request_body_can_be_parsed_as_form() {
    let mut app = App::new();

    app.post("/formulario", |request, response| {
        let campos = request.form();

        assert_eq!(
            campos,
            vec![
                ("nome".to_string(), "Fabricio".to_string()),
                ("email".to_string(), "fabricio@example.com".to_string()),
                ("interesses".to_string(), "Rust".to_string()),
                ("interesses".to_string(), "PHP".to_string()),
                ("vazio".to_string(), "".to_string()),
            ]
        );

        Ok(response.send("Formulario processado"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_body(
        app,
        "POST",
        "/formulario",
        b"nome=Fabricio&email=fabricio%40example.com&interesses=Rust&interesses=PHP&vazio=",
    );

    assert!(
        response.contains("200 OK"),
        "Código HTTP inesperado: {response}"
    );

    assert!(response.contains("Formulario processado"));
}

#[test]
fn request_can_read_cookie() {
    let mut app = App::new();

    app.get("/cookie", |req: Request, _res: Response| {
        let value = req.cookie("usuario").unwrap().unwrap();

        Ok(Response::new().send(value))
    })
    .unwrap();

    let response = request_with_headers(app, "GET", "/cookie", &[("Cookie", "usuario=fabricio")]);

    assert!(response.contains("fabricio"));
}

#[test]
fn request_returns_none_for_missing_cookie() {
    let mut app = App::new();

    app.get("/cookie", |req: Request, _res: Response| {
        let value = req.cookie("usuario").unwrap();

        let message = if value.is_none() {
            "Cookie não encontrado"
        } else {
            "Cookie encontrado"
        };

        Ok(Response::new().send(message))
    })
    .unwrap();

    let response = request_with_headers(app, "GET", "/cookie", &[]);

    assert!(response.contains("Cookie não encontrado"));
}

#[test]
fn request_rejects_duplicate_cookies() {
    let mut app = App::new();

    app.get("/cookie", |req: Request, _res: Response| {
        let result = req.cookie("usuario");

        let message = match result {
            Err(_) => "Cookie duplicado ou inválido",
            Ok(Some(_)) => "Cookie encontrado",
            Ok(None) => "Cookie não encontrado",
        };

        Ok(Response::new().send(message))
    })
    .unwrap();

    let response = request_with_headers(
        app,
        "GET",
        "/cookie",
        &[("Cookie", "usuario=fabricio; usuario=paulo")],
    );

    assert!(response.contains("Cookie duplicado ou inválido"));
}

#[test]
fn request_can_read_cookie_from_multiple_headers() {
    let mut app = App::new();

    app.get("/cookie", |req: Request, _res: Response| {
        let value = req.cookie("tema").unwrap().unwrap();

        Ok(Response::new().send(value))
    })
    .unwrap();

    let response = request_with_headers(
        app,
        "GET",
        "/cookie",
        &[("Cookie", "usuario=fabricio"), ("Cookie", "tema=escuro")],
    );

    assert!(response.contains("escuro"));
}

#[test]
fn request_can_read_all_cookies() {
    let mut app = App::new();

    app.get("/cookies", |req: Request, _res: Response| {
        let cookies = req.cookies().unwrap();

        let result = cookies
            .iter()
            .map(|cookie| format!("{}={}", cookie.name, cookie.value))
            .collect::<Vec<String>>()
            .join(", ");

        Ok(Response::new().send(result))
    })
    .unwrap();

    let response = request_with_headers(
        app,
        "GET",
        "/cookies",
        &[
            ("Cookie", "usuario=fabricio; tema=escuro"),
            ("Cookie", "idioma=pt"),
        ],
    );

    assert!(response.contains("usuario=fabricio"));
    assert!(response.contains("tema=escuro"));
    assert!(response.contains("idioma=pt"));
}

#[test]
fn redirect_returns_302_with_location_header() {
    let mut app = App::new();

    app.get("/old-page", |_req: Request, _res| {
        Ok(Response::redirect("/new-page"))
    })
    .expect("Não foi possível registar a rota");

    let response = request_with_app(app, "GET", "/old-page");

    assert!(response.starts_with("HTTP/1.1 302"));
    assert!(response.to_lowercase().contains("location: /new-page"));
}

#[test]
fn auth_middleware_protects_route() {
    fn create_app() -> App {
        let mut app = App::new();

        let auth = AuthMiddleware::new(|request: &Request| {
            let authenticated = request
                .header("authorization")
                .and_then(|value| value.to_str().ok())
                == Some("Bearer token-valido");

            Ok(authenticated)
        });

        app.use_middleware_advanced(move |request| auth.handle(request));

        app.get("/dashboard", |_request, response| {
            Ok(response.send("Bem-vindo ao dashboard"))
        })
        .expect("Não foi possível registar a rota");

        app
    }

    // Primeiro cenário: pedido sem credenciais.
    let mut app = create_app();

    let response = request_with_app(app, "GET", "/dashboard");

    assert!(
        response.contains("401"),
        "Era esperada uma resposta HTTP 401: {response}"
    );

    assert!(
        !response.contains("Bem-vindo ao dashboard"),
        "O utilizador não autenticado acedeu indevidamente à rota"
    );

    // Segundo cenário: pedido com credenciais válidas.
    app = create_app();

    let response = request_with_headers(
        app,
        "GET",
        "/dashboard",
        &[("Authorization", "Bearer token-valido")],
    );

    assert!(
        response.contains("200 OK"),
        "Era esperada uma resposta HTTP 200: {response}"
    );

    assert!(
        response.contains("Bem-vindo ao dashboard"),
        "O utilizador autenticado não conseguiu aceder à rota"
    );
}

#[test]
fn role_middleware_allows_authorized_user() {
    let mut app = App::new();

    app.use_sessions(SessionConfig::default()).unwrap();

    let role = RoleMiddleware::new("admin");

    app.use_middleware_advanced(move |request: &Request| {
        if request.path() == "/admin" {
            role.handle(request)
        } else {
            MiddlewareResult::Next
        }
    });

    app.post("/login", |request, response| {
        let session = request.session().expect("A sessão não foi disponibilizada");

        let mut session = session.lock().unwrap();

        session.insert("user_id", 1_u64).unwrap();
        session.insert("role", "admin").unwrap();

        Ok(response.send("Sessão iniciada"))
    })
    .unwrap();

    app.get("/admin", |_request, response| {
        Ok(response.send("Área administrativa"))
    })
    .unwrap();

    let (login_response, admin_response) =
        request_then_with_cookie(app, "POST", "/login", "GET", "/admin");

    assert!(login_response.contains("200 OK"));
    assert!(login_response.contains("Sessão iniciada"));

    assert!(admin_response.contains("200 OK"));
    assert!(admin_response.contains("Área administrativa"));
}

#[test]
fn role_middleware_rejects_unauthorized_role() {
    let mut app = App::new();

    app.use_sessions(SessionConfig::default()).unwrap();

    let role = RoleMiddleware::new("admin");

    app.use_middleware_advanced(move |request: &Request| {
        if request.path() == "/admin" {
            role.handle(request)
        } else {
            MiddlewareResult::Next
        }
    });

    app.post("/login", |request, response| {
        let session = request.session().expect("A sessão não foi disponibilizada");

        let mut session = session.lock().unwrap();

        session.insert("user_id", 2_u64).unwrap();
        session.insert("role", "professor").unwrap();

        Ok(response.send("Login efectuado"))
    })
    .unwrap();

    app.get("/admin", |_request, response| {
        Ok(response.send("Área administrativa"))
    })
    .unwrap();

    let (_, admin_response) = request_then_with_cookie(app, "POST", "/login", "GET", "/admin");

    assert!(admin_response.contains("403 Forbidden"));
}

#[test]
fn role_middleware_rejects_unauthenticated_user() {
    let mut app = App::new();

    app.use_sessions(SessionConfig::default()).unwrap();

    let role = RoleMiddleware::new("admin");

    app.use_middleware_advanced(move |request: &Request| {
        if request.path() == "/admin" {
            role.handle(request)
        } else {
            MiddlewareResult::Next
        }
    });

    app.post("/login", |request, response| {
        let session = request.session().expect("A sessão não foi disponibilizada");

        let mut session = session.lock().unwrap();

        // Cria uma sessão modificada, mas sem user_id.
        session.insert("role", "guest").unwrap();

        Ok(response.send("Sem autenticação"))
    })
    .unwrap();

    app.get("/admin", |_request, response| {
        Ok(response.send("Área administrativa"))
    })
    .unwrap();

    let (_, admin_response) = request_then_with_cookie(app, "POST", "/login", "GET", "/admin");

    assert!(admin_response.contains("401 Unauthorized"));
}
