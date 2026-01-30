//! Collaboration client module.
//!
//! This module provides client-side functionality for collaborative editing,
//! including connecting to servers, sending updates, and receiving updates
//! from other clients.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};

use crate::network::{ClientMessage, ServerMessage};

/// Client state for collaborative editing.
pub struct CollaborationClient {
    /// The TCP stream to the server.
    writer: tokio::net::tcp::OwnedWriteHalf,
    /// Current state of all clients' code.
    pub clients: Arc<RwLock<HashMap<String, String>>>,
}

impl CollaborationClient {
    /// Connects to a collaboration server.
    /// Returns the client, an update notification receiver, and a handle to the reader task.
    pub async fn connect(
        server_addr: &str,
    ) -> Result<(Self, mpsc::Receiver<()>, tokio::task::JoinHandle<()>)> {
        let stream = TcpStream::connect(server_addr).await?;
        println!("Connected to collaboration server at {}", server_addr);

        let (reader, writer) = stream.into_split();
        let clients: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));
        let clients_clone = Arc::clone(&clients);

        // Channel to notify when updates are received
        let (update_tx, update_rx) = mpsc::channel::<()>(16);

        // Spawn a task to handle incoming messages
        let handle = tokio::spawn(async move {
            let mut reader = BufReader::new(reader);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        eprintln!("Server disconnected");
                        break;
                    }
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<ServerMessage>(&line) {
                            match msg {
                                ServerMessage::ClientUpdate { client_ip, code } => {
                                    println!("Received update from client {}", client_ip);
                                    let mut state = clients_clone.write().await;
                                    state.insert(client_ip, code);
                                }
                                ServerMessage::ClientDisconnected { client_ip } => {
                                    println!("Client {} disconnected", client_ip);
                                    let mut state = clients_clone.write().await;
                                    state.remove(&client_ip);
                                }
                            }
                            // Notify that an update was received
                            let _ = update_tx.send(()).await;
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from server: {}", e);
                        break;
                    }
                }
            }
        });

        Ok((Self { writer, clients }, update_rx, handle))
    }

    /// Sends a code update to the server.
    pub async fn send_code_update(&mut self, code: String) -> Result<()> {
        let msg = ClientMessage::CodeUpdate { code };
        let json = serde_json::to_string(&msg)? + "\n";
        self.writer.write_all(json.as_bytes()).await?;
        Ok(())
    }
}
