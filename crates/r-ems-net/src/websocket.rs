//! ---
//! ems_section: "05-networking-external-interfaces"
//! ems_subsection: "module"
//! ems_type: "source"
//! ems_scope: "code"
//! ems_description: "Network connectivity and edge adapters."
//! ems_version: "v0.0.0-prealpha"
//! ems_owner: "tbd"
//! ---
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// Telemetry frame distributed to subscribed WebSocket clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TelemetryFrame {
    /// Logical channel name (e.g. grid/controller identifier).
    pub channel: String,
    /// Arbitrary payload encoded as JSON.
    pub payload: serde_json::Value,
}

impl TelemetryFrame {
    /// Convenience constructor for structured payloads.
    pub fn new(channel: impl Into<String>, payload: serde_json::Value) -> Self {
        Self {
            channel: channel.into(),
            payload,
        }
    }
}

/// Broadcasts telemetry frames to all connected clients.
#[derive(Clone)]
pub struct TelemetryBroadcaster {
    tx: broadcast::Sender<TelemetryFrame>,
}

impl TelemetryBroadcaster {
    /// Create a broadcaster with the specified channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Send a frame to subscribers. Returns the number of peers reached.
    pub fn send(
        &self,
        frame: TelemetryFrame,
    ) -> Result<usize, broadcast::error::SendError<TelemetryFrame>> {
        self.tx.send(frame)
    }

    fn subscribe(&self) -> broadcast::Receiver<TelemetryFrame> {
        self.tx.subscribe()
    }
}

struct WebSocketState {
    broadcaster: TelemetryBroadcaster,
}

/// Builder for the WebSocket server that streams telemetry updates.
#[derive(Clone)]
pub struct WebSocketServerBuilder {
    listen: SocketAddr,
    broadcaster: TelemetryBroadcaster,
}

impl WebSocketServerBuilder {
    /// Create a builder bound to `listen` using the provided broadcaster.
    pub fn new(listen: SocketAddr, broadcaster: TelemetryBroadcaster) -> Self {
        Self {
            listen,
            broadcaster,
        }
    }

    /// Spawn the WebSocket server and return a shutdown handle.
    pub async fn spawn(self) -> anyhow::Result<WebSocketServerHandle> {
        let listener = TcpListener::bind(self.listen).await?;
        let local_addr = listener.local_addr()?;
        info!(address = %local_addr, "websocket server listening");

        let state = Arc::new(WebSocketState {
            broadcaster: self.broadcaster,
        });

        let app = Router::new()
            .route("/ws", get(upgrade_handler))
            .with_state(state);

        let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let server = axum::serve(listener, app).with_graceful_shutdown(async move {
                let _ = shutdown_rx.changed().await;
            });
            if let Err(err) = server.await {
                warn!(error = %err, "websocket server exited with error");
            }
        });

        Ok(WebSocketServerHandle {
            address: local_addr,
            shutdown: shutdown_tx,
            task,
        })
    }
}

/// Handle for the running WebSocket server.
pub struct WebSocketServerHandle {
    address: SocketAddr,
    shutdown: watch::Sender<bool>,
    task: JoinHandle<()>,
}

impl WebSocketServerHandle {
    /// Return the bound listening address.
    pub fn local_addr(&self) -> SocketAddr {
        self.address
    }

    /// Trigger graceful shutdown and await completion.
    pub async fn shutdown(self) -> anyhow::Result<()> {
        let _ = self.shutdown.send(true);
        match self.task.await {
            Ok(()) => Ok(()),
            Err(err) => Err(anyhow::anyhow!(err)),
        }
    }
}

#[derive(Debug, Deserialize)]
struct ClientCommand {
    action: String,
    #[serde(default)]
    channels: HashSet<String>,
}

async fn upgrade_handler(
    ws: WebSocketUpgrade,
    State(state): State<Arc<WebSocketState>>,
) -> axum::response::Response {
    ws.on_upgrade(|socket| client_loop(socket, state))
}

async fn client_loop(mut socket: WebSocket, state: Arc<WebSocketState>) {
    let mut subscription = state.broadcaster.subscribe();
    let mut allowed_channels: Option<HashSet<String>> = None;

    loop {
        tokio::select! {
            frame = subscription.recv() => {
                let frame = match frame {
                    Ok(frame) => frame,
                    Err(broadcast::error::RecvError::Lagged(skipped)) => {
                        warn!(skipped, "websocket client lagged behind; dropping frames");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                if let Some(channels) = &allowed_channels {
                    if !channels.contains(&frame.channel) {
                        continue;
                    }
                }

                let Ok(text) = serde_json::to_string(&frame) else {
                    warn!("failed to serialise telemetry frame");
                    continue;
                };

                if socket.send(Message::Text(text)).await.is_err() {
                    break;
                }
            }
            message = socket.recv() => {
                let Some(Ok(message)) = message else {
                    break;
                };

                match message {
                    Message::Text(text) => {
                        match serde_json::from_str::<ClientCommand>(&text) {
                            Ok(cmd) => handle_command(cmd, &mut allowed_channels),
                            Err(err) => {
                                warn!(error = %err, "invalid websocket command payload");
                                let _ = socket
                                    .send(Message::Text("{\"error\":\"invalid command\"}".into()))
                                    .await;
                            }
                        }
                    }
                    Message::Binary(_) => {
                        let _ = socket
                            .send(Message::Text("{\"error\":\"binary unsupported\"}".into()))
                            .await;
                    }
                    Message::Ping(payload) => {
                        if socket.send(Message::Pong(payload)).await.is_err() {
                            break;
                        }
                    }
                    Message::Pong(_) => {}
                    Message::Close(_) => break,
                }
            }
        }
    }
}

fn handle_command(cmd: ClientCommand, allowed_channels: &mut Option<HashSet<String>>) {
    match cmd.action.as_str() {
        "subscribe" => {
            let mut set = allowed_channels.take().unwrap_or_default();
            set.extend(cmd.channels);
            *allowed_channels = Some(set);
        }
        "unsubscribe" => {
            if let Some(set) = allowed_channels.as_mut() {
                for channel in cmd.channels {
                    set.remove(&channel);
                }
            }
        }
        "unsubscribe_all" => {
            *allowed_channels = None;
        }
        _ => warn!(action = %cmd.action, "unknown websocket command received"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use serde_json::json;
    use tokio::time::{sleep, timeout, Duration};
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as WsMessage};

    #[tokio::test]
    async fn websocket_broadcasts_follow_subscriptions() {
        let broadcaster = TelemetryBroadcaster::new(16);
        let builder =
            WebSocketServerBuilder::new("127.0.0.1:0".parse().unwrap(), broadcaster.clone());
        let handle = builder.spawn().await.unwrap();
        let url = format!("ws://{}/ws", handle.local_addr());

        let (mut socket, _response) = connect_async(&url).await.unwrap();

        // Subscribe to channel "grid-a".
        socket
            .send(WsMessage::Text(
                json!({
                    "action": "subscribe",
                    "channels": ["grid-a"]
                })
                .to_string(),
            ))
            .await
            .unwrap();

        sleep(Duration::from_millis(20)).await;

        // Send two frames, only one should be forwarded based on subscription.
        broadcaster
            .send(TelemetryFrame::new("grid-a", json!({"power": 42})))
            .unwrap();
        broadcaster
            .send(TelemetryFrame::new("grid-b", json!({"power": 10})))
            .unwrap();

        let received = socket.next().await.unwrap().unwrap();
        match received {
            WsMessage::Text(payload) => {
                let frame: TelemetryFrame = serde_json::from_str(&payload).unwrap();
                assert_eq!(frame.channel, "grid-a");
                assert_eq!(frame.payload, json!({"power": 42}));
            }
            other => panic!("unexpected message: {other:?}"),
        }

        // No additional message should arrive.
        assert!(timeout(Duration::from_millis(50), socket.next())
            .await
            .is_err());

        sleep(Duration::from_millis(10)).await;
        handle.shutdown().await.unwrap();
    }
}
