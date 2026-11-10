use futures_util::{SinkExt, StreamExt};
use rustpress_web::App;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;

#[tokio::test]
async fn accepts_websocket_handshake() {
    let mut app = App::new();

    app.ws("/ws", |_, mut websocket| async move {
        if let Ok(Some(rustpress_web::Message::Text(text))) = websocket.recv().await {
            let _ = websocket.send(rustpress_web::Message::Text(text)).await;
        }
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("O handshake WebSocket deveria ser aceite");

    assert_eq!(response.status(), 101);

    websocket
        .send(tokio_tungstenite::tungstenite::Message::Text(
            "Olá RustPress!".into(),
        ))
        .await
        .expect("Não foi possível enviar a mensagem");

    let received = websocket
        .next()
        .await
        .expect("O servidor não devolveu uma mensagem")
        .expect("Erro ao receber a mensagem");

    assert_eq!(
        received,
        tokio_tungstenite::tungstenite::Message::Text("Olá RustPress!".into())
    );

    websocket.close(None).await.unwrap();
}

#[tokio::test]
async fn supports_dynamic_websocket_route_parameters() {
    let mut app = App::new();

    app.ws("/ws/:id", |request, mut websocket| async move {
        if let Some(id) = request.param("id") {
            let _ = websocket
                .send(rustpress_web::Message::Text(id.to_string()))
                .await;
        }
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws/123");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("A rota WebSocket dinâmica deveria ser aceite");

    assert_eq!(response.status(), 101);

    let received = websocket
        .next()
        .await
        .expect("O servidor não devolveu uma mensagem")
        .expect("Erro ao receber a mensagem");

    assert_eq!(
        received,
        tokio_tungstenite::tungstenite::Message::Text("123".into())
    );

    websocket.close(None).await.unwrap();
}

#[tokio::test]
async fn global_middleware_can_reject_websocket_handshake() {
    let mut app = App::new();

    app.use_middleware(|_| {
        Some(rustpress_web::Response::error(
            403,
            "Acesso WebSocket proibido",
        ))
    });

    app.ws("/ws", |_, _| async {})
        .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let result = connect_async(url).await;

    assert!(
        result.is_err(),
        "O handshake deveria ser rejeitado pelo middleware"
    );

    assert!(
        matches!(
            result,
            Err(tokio_tungstenite::tungstenite::Error::Http(response))
                if response.status().as_u16() == 403
        ),
        "O servidor deveria rejeitar o handshake com HTTP 403"
    );
}

#[tokio::test]
async fn supports_binary_websocket_messages() {
    let mut app = App::new();

    app.ws("/ws", |_, mut websocket| async move {
        if let Ok(Some(rustpress_web::Message::Binary(data))) = websocket.recv().await {
            let _ = websocket.send(rustpress_web::Message::Binary(data)).await;
        }
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("O handshake WebSocket deveria ser aceite");

    assert_eq!(response.status(), 101);

    let payload = vec![0, 1, 2, 3, 127, 128, 255];

    websocket
        .send(tokio_tungstenite::tungstenite::Message::Binary(
            payload.clone().into(),
        ))
        .await
        .expect("Não foi possível enviar a mensagem binária");

    let received = websocket
        .next()
        .await
        .expect("O servidor não devolveu uma mensagem")
        .expect("Erro ao receber a mensagem");

    assert_eq!(
        received,
        tokio_tungstenite::tungstenite::Message::Binary(payload.into())
    );

    websocket.close(None).await.unwrap();
}

#[tokio::test]
async fn supports_websocket_connection_close() {
    let mut app = App::new();

    app.ws("/ws", |_, mut websocket| async move {
        let _ = websocket.send(rustpress_web::Message::Close).await;
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("O handshake WebSocket deveria ser aceite");

    assert_eq!(response.status(), 101);

    let received = websocket
        .next()
        .await
        .expect("O servidor não enviou uma mensagem de encerramento")
        .expect("Erro ao receber a mensagem de encerramento");

    assert!(matches!(
        received,
        tokio_tungstenite::tungstenite::Message::Close(_)
    ));
}

#[tokio::test]
async fn recv_returns_none_after_client_disconnects() {
    let mut app = App::new();

    app.ws("/ws", |_, mut websocket| async move {
        let result = websocket.recv().await;

        assert!(
            matches!(result, Ok(Some(rustpress_web::Message::Close))),
            "O servidor deveria receber a mensagem de encerramento do cliente"
        );

        websocket
            .close()
            .await
            .expect("O servidor deveria responder ao encerramento do cliente");
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("O handshake WebSocket deveria ser aceite");

    assert_eq!(response.status(), 101);

    websocket.close(None).await.unwrap();

    let received = tokio::time::timeout(std::time::Duration::from_secs(2), websocket.next())
        .await
        .expect("O servidor não respondeu ao encerramento dentro do prazo")
        .expect("A ligação foi encerrada sem resposta")
        .expect("Erro ao receber a resposta de encerramento");

    assert!(matches!(
        received,
        tokio_tungstenite::tungstenite::Message::Close(_)
    ));
}

#[tokio::test]
async fn supports_websocket_ping_messages() {
    let mut app = App::new();

    app.ws("/ws", |_, mut websocket| async move {
        if let Ok(Some(rustpress_web::Message::Ping(data))) = websocket.recv().await {
            let _ = websocket.send(rustpress_web::Message::Pong(data)).await;
        }
    })
    .expect("Não foi possível registar a rota WebSocket");

    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Não foi possível reservar uma porta");

    let address = listener.local_addr().unwrap();

    tokio::spawn(async move {
        app.serve_listener(listener)
            .await
            .expect("O servidor RustPress falhou");
    });

    let url = format!("ws://{address}/ws");

    let (mut websocket, response) = connect_async(url)
        .await
        .expect("O handshake WebSocket deveria ser aceite");

    assert_eq!(response.status(), 101);

    let payload = vec![1, 2, 3, 4];

    websocket
        .send(tokio_tungstenite::tungstenite::Message::Ping(
            payload.clone().into(),
        ))
        .await
        .expect("Não foi possível enviar Ping");

    let received = websocket
        .next()
        .await
        .expect("O servidor não devolveu uma mensagem")
        .expect("Erro ao receber a mensagem");

    assert_eq!(
        received,
        tokio_tungstenite::tungstenite::Message::Pong(payload.into())
    );

    websocket.close(None).await.unwrap();
}
