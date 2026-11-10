//! Tipos públicos e funcionalidades de comunicação WebSocket.

/// Representa uma mensagem recebida ou enviada através de uma ligação WebSocket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// Mensagem de texto UTF-8.
    Text(String),

    /// Mensagem binária.
    Binary(Vec<u8>),

    /// Mensagem de controlo Ping.
    Ping(Vec<u8>),

    /// Mensagem de controlo Pong.
    Pong(Vec<u8>),

    /// Encerramento da ligação.
    Close,
}

use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::{HyperWebsocket, HyperWebsocketStream};

/// Representa uma ligação WebSocket activa.
pub struct WebSocket {
    stream: HyperWebsocketStream,
}

/// Erros que podem ocorrer durante uma comunicação WebSocket.
pub type WebSocketError = hyper_tungstenite::tungstenite::Error;

impl WebSocket {
    /// Cria uma ligação WebSocket a partir de um pedido HTTP actualizado.
    pub async fn from_upgrade(upgrade: HyperWebsocket) -> Result<Self, WebSocketError> {
        let stream = upgrade.await?;

        Ok(Self { stream })
    }

    /// Envia uma mensagem através da ligação WebSocket.
    pub async fn send(&mut self, message: Message) -> Result<(), WebSocketError> {
        use hyper_tungstenite::tungstenite::Message as TungsteniteMessage;

        let message = match message {
            Message::Text(text) => TungsteniteMessage::Text(text.into()),
            Message::Binary(data) => TungsteniteMessage::Binary(data.into()),
            Message::Ping(data) => TungsteniteMessage::Ping(data.into()),
            Message::Pong(data) => TungsteniteMessage::Pong(data.into()),
            Message::Close => TungsteniteMessage::Close(None),
        };

        self.stream.send(message).await
    }

    /// Recebe a próxima mensagem da ligação WebSocket.
    ///
    /// Devolve `Ok(None)` quando a ligação é encerrada.
    pub async fn recv(&mut self) -> Result<Option<Message>, WebSocketError> {
        use hyper_tungstenite::tungstenite::Message as TungsteniteMessage;

        loop {
            match self.stream.next().await {
                Some(Ok(message)) => {
                    let message = match message {
                        TungsteniteMessage::Text(text) => Message::Text(text.to_string()),
                        TungsteniteMessage::Binary(data) => Message::Binary(data.to_vec()),
                        TungsteniteMessage::Ping(data) => Message::Ping(data.to_vec()),
                        TungsteniteMessage::Pong(data) => Message::Pong(data.to_vec()),
                        TungsteniteMessage::Close(_) => {
                            self.stream.flush().await?;
                            return Ok(Some(Message::Close));
                        }
                        TungsteniteMessage::Frame(_) => continue,
                    };

                    return Ok(Some(message));
                }
                Some(Err(error)) => return Err(error),
                None => return Ok(None),
            }
        }
    }

    /// Encerra a ligação WebSocket de forma controlada.
    pub async fn close(&mut self) -> Result<(), WebSocketError> {
        self.stream.close(None).await
    }
}
