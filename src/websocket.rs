//! Public types and WebSocket communication features.

/// Represents a message received or sent through a WebSocket connection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// UTF-8 text message.
    Text(String),

    /// Binary message.
    Binary(Vec<u8>),

    /// Ping control message.
    Ping(Vec<u8>),

    /// Pong control message.
    Pong(Vec<u8>),

    /// Connection closure.
    Close,
}

use futures_util::{SinkExt, StreamExt};
use hyper_tungstenite::{HyperWebsocket, HyperWebsocketStream};

/// Represents an active WebSocket connection.
pub struct WebSocket {
    stream: HyperWebsocketStream,
}

/// Errors that can occur during WebSocket communication.
pub type WebSocketError = hyper_tungstenite::tungstenite::Error;

impl WebSocket {
    /// Creates a WebSocket connection from an upgraded HTTP request.
    pub async fn from_upgrade(upgrade: HyperWebsocket) -> Result<Self, WebSocketError> {
        let stream = upgrade.await?;

        Ok(Self { stream })
    }

    /// Sends a message through the WebSocket connection.
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

    /// Receives the next message from the WebSocket connection.
    ///
    /// Returns `Ok(None)` when the connection is closed.
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

    /// Closes the WebSocket connection gracefully.
    pub async fn close(&mut self) -> Result<(), WebSocketError> {
        self.stream.close(None).await
    }
}
