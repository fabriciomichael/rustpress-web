use rustpress_web::{App, HandlerError, MiddlewareResult, Response};

use std::{
    io::{Read, Write},
    net::TcpStream,
    thread,
    time::{Duration, Instant},
};
use tokio::net::TcpListener;

fn request_many_with_app(app: App, requests: &[(&str, &str)]) -> Vec<String> {
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

    let mut responses = Vec::new();

    for (method, path) in requests {
        loop {
            if Instant::now() >= deadline {
                panic!("O servidor não ficou disponível a tempo");
            }

            if let Ok(mut stream) = TcpStream::connect_timeout(&address, Duration::from_millis(200))
            {
                let request = format!(
                    "{method} {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
                );

                stream.write_all(request.as_bytes()).unwrap();

                let mut response = String::new();
                stream.read_to_string(&mut response).unwrap();

                responses.push(response);
                break;
            }

            thread::sleep(Duration::from_millis(50));
        }
    }

    responses
}

fn request_twice_with_app(
    app: App,
    first_method: &str,
    first_path: &str,
    second_method: &str,
    second_path: &str,
) -> (String, String) {
    let mut responses = request_many_with_app(
        app,
        &[(first_method, first_path), (second_method, second_path)],
    );

    (responses.remove(0), responses.remove(0))
}

#[test]
fn route_group_applies_prefix_and_isolates_group_middleware() {
    let mut app = App::new();

    app.group("/admin", |admin| {
        admin.get("/dashboard", |_request, response| {
            Ok(response.send("Dashboard administrativo"))
        })?;

        admin.use_middleware_advanced(|_request| {
            MiddlewareResult::Respond(Response::new().send("Acesso bloqueado"))
        });

        Ok(())
    })
    .expect("Não foi possível registar o grupo");

    app.get("/publico", |_request, response| {
        Ok(response.send("Página pública"))
    })
    .expect("Não foi possível registar a rota pública");

    let (admin_response, public_response) =
        request_twice_with_app(app, "GET", "/admin/dashboard", "GET", "/publico");

    assert!(admin_response.contains("Acesso bloqueado"));
    assert!(!admin_response.contains("Dashboard administrativo"));

    assert!(public_response.contains("Página pública"));
    assert!(!public_response.contains("Acesso bloqueado"));
}

#[test]
fn failed_route_group_configuration_does_not_register_routes() {
    let mut app = App::new();

    let result = app.group("/admin", |admin| {
        admin.get("/dashboard", |_request, response| {
            Ok(response.send("Dashboard"))
        })?;

        Err("Erro simulado na configuração".to_owned())
    });

    assert!(result.is_err());

    let (response, _) =
        request_twice_with_app(app, "GET", "/admin/dashboard", "GET", "/rota-inexistente");

    assert!(response.contains("404"));
    assert!(!response.contains("Dashboard"));
}

#[test]
fn nested_route_groups_execute_middlewares_in_correct_order() {
    let mut app = App::new();

    let execution_order = std::sync::Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));

    // Middleware global.
    let global_order = std::sync::Arc::clone(&execution_order);

    app.use_middleware_advanced(move |_request| {
        global_order.lock().unwrap().push("global");
        MiddlewareResult::Next
    });

    app.group("/admin", |admin| {
        // Criamos primeiro o grupo interior.
        admin.group("/utilizadores", |users| {
            let inner_order = std::sync::Arc::clone(&execution_order);

            users.use_middleware_advanced(move |_request| {
                inner_order.lock().unwrap().push("interior");
                MiddlewareResult::Next
            });

            let handler_order = std::sync::Arc::clone(&execution_order);

            users.get("/lista", move |_request, response| {
                handler_order.lock().unwrap().push("handler");
                Ok(response.send("Lista de utilizadores"))
            })?;

            Ok(())
        })?;

        // Registamos o middleware exterior depois do grupo interior.
        let outer_order = std::sync::Arc::clone(&execution_order);

        admin.use_middleware_advanced(move |_request| {
            outer_order.lock().unwrap().push("exterior");
            MiddlewareResult::Next
        });

        Ok(())
    })
    .expect("Não foi possível registar os grupos");

    let (response, _) = request_twice_with_app(
        app,
        "GET",
        "/admin/utilizadores/lista",
        "GET",
        "/rota-inexistente",
    );

    assert!(response.contains("Lista de utilizadores"));

    let actual_order = execution_order
        .lock()
        .unwrap()
        .iter()
        .take(4)
        .copied()
        .collect::<Vec<_>>();

    assert_eq!(
        actual_order,
        vec!["global", "exterior", "interior", "handler"],
        "Os middlewares não foram executados na ordem esperada"
    );
}

#[test]
fn route_group_supports_all_http_methods() {
    let mut app = App::new();

    app.group("/api", |api| {
        api.get("/recurso", |_request, response| Ok(response.send("GET OK")))?;

        api.post(
            "/recurso",
            |_request, response| Ok(response.send("POST OK")),
        )?;

        api.put("/recurso", |_request, response| response.send("PUT OK"))?;

        api.patch("/recurso", |_request, response| response.send("PATCH OK"))?;

        api.delete("/recurso", |_request, response| response.send("DELETE OK"))?;

        Ok(())
    })
    .expect("Não foi possível registar as rotas HTTP");

    let responses = request_many_with_app(
        app,
        &[
            ("GET", "/api/recurso"),
            ("POST", "/api/recurso"),
            ("PUT", "/api/recurso"),
            ("PATCH", "/api/recurso"),
            ("DELETE", "/api/recurso"),
        ],
    );

    assert_eq!(responses.len(), 5);

    assert!(responses[0].contains("GET OK"));
    assert!(responses[1].contains("POST OK"));
    assert!(responses[2].contains("PUT OK"));
    assert!(responses[3].contains("PATCH OK"));
    assert!(responses[4].contains("DELETE OK"));
}

#[test]
fn route_group_fallible_handlers_propagate_errors() {
    let mut app = App::new();

    app.set_error_handler(|error, _context| {
        Response::new()
            .status(error.status())
            .send(error.public_message())
    });

    app.group("/api", |api| {
        api.put_fallible("/recurso", |_request, _response| {
            Err(HandlerError::with_status(422, "Erro no PUT"))
        })?;

        api.patch_fallible("/recurso", |_request, _response| {
            Err(HandlerError::with_status(422, "Erro no PATCH"))
        })?;

        api.delete_fallible("/recurso", |_request, _response| {
            Err(HandlerError::with_status(422, "Erro no DELETE"))
        })?;

        Ok(())
    })
    .expect("Não foi possível registar os handlers fallible");

    let responses = request_many_with_app(
        app,
        &[
            ("PUT", "/api/recurso"),
            ("PATCH", "/api/recurso"),
            ("DELETE", "/api/recurso"),
        ],
    );

    assert_eq!(responses.len(), 3);

    assert!(responses[0].starts_with("HTTP/1.1 422"));
    assert!(responses[0].contains("Erro no PUT"));

    assert!(responses[1].starts_with("HTTP/1.1 422"));
    assert!(responses[1].contains("Erro no PATCH"));

    assert!(responses[2].starts_with("HTTP/1.1 422"));
    assert!(responses[2].contains("Erro no DELETE"));
}

#[test]
fn route_group_conflict_with_existing_route_does_not_register_any_route() {
    let mut app = App::new();

    app.get("/admin/existente", |_request, response| {
        Ok(response.send("Rota original"))
    })
    .expect("Não foi possível registar a rota original");

    let result = app.group("/admin", |admin| {
        admin.get("/nova", |_request, response| Ok(response.send("Rota nova")))?;

        admin.get("/existente", |_request, response| {
            Ok(response.send("Rota substituída"))
        })?;

        Ok(())
    });

    assert!(result.is_err());

    let responses =
        request_many_with_app(app, &[("GET", "/admin/existente"), ("GET", "/admin/nova")]);

    assert!(responses[0].contains("Rota original"));
    assert!(!responses[0].contains("Rota substituída"));
    assert!(responses[1].contains("404"));
    assert!(!responses[1].contains("Rota nova"));
}

#[test]
fn route_group_duplicate_routes_do_not_register_any_route() {
    let mut app = App::new();

    let result = app.group("/api", |api| {
        api.get("/primeiro", |_request, response| {
            Ok(response.send("Primeira rota"))
        })?;

        api.get("/duplicado", |_request, response| {
            Ok(response.send("Primeira definição"))
        })?;

        api.get("/duplicado", |_request, response| {
            Ok(response.send("Segunda definição"))
        })?;

        Ok(())
    });

    assert!(result.is_err());

    let responses =
        request_many_with_app(app, &[("GET", "/api/primeiro"), ("GET", "/api/duplicado")]);

    assert!(responses[0].contains("404"));
    assert!(responses[1].contains("404"));
}
