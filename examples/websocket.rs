use rustpress_web::{App, Message, Request, WebSocket};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut app = App::new();

    app.ws(
        "/ws",
        |_request: Request, mut websocket: WebSocket| async move {
            loop {
                match websocket.recv().await {
                    Ok(Some(Message::Text(text))) => {
                        if websocket.send(Message::Text(text)).await.is_err() {
                            break;
                        }
                    }
                    Ok(Some(Message::Close)) => {
                        if let Err(error) = websocket.close().await {
                            eprintln!("Erro ao encerrar WebSocket: {error}");
                        }
                        break;
                    }
                    Ok(None) => {
                        break;
                    }
                    Ok(Some(_)) => {}
                    Err(error) => {
                        eprintln!("Erro WebSocket: {error}");
                        break;
                    }
                }
            }
        },
    )?;

    app.listen("127.0.0.1:3000").await?;

    Ok(())
}
