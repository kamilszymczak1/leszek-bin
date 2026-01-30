//! Network module for collaborative pattern editing.
//!
//! This module provides a server and client implementation that allows
//! multiple users to work on patterns simultaneously. The server maintains
//! a map of client IPs to their code files and broadcasts updates to all
//! connected clients.
//!
//! Architecture:
//! - Client handlers receive updates and push them to a central update queue
//! - A single broadcaster task reads from the queue and sends updates to all clients
//! - Each client maintains its own local state by applying incoming updates

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{RwLock, broadcast, mpsc};

/// Messages sent from client to server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Client sends their code update to the server.
    CodeUpdate { code: String },
}

/// Messages sent from server to client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Server sends an individual client's update to all clients.
    ClientUpdate { client_ip: String, code: String },
    /// Server notifies that a client disconnected.
    ClientDisconnected { client_ip: String },
}

/// An update to be broadcast to all clients.
#[derive(Debug, Clone)]
enum Update {
    /// A client updated their code.
    Code { client_ip: String, code: String },
    /// A client disconnected.
    Disconnected { client_ip: String },
}

/// Runs the collaboration server.
///
/// The server listens for client connections and maintains a map of
/// client IPs to their code. Updates are queued and broadcast by a
/// dedicated task.
pub async fn run_server(addr: &str) -> Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!("Collaboration server listening on {}", addr);

    // Channel for client handlers to send updates to the broadcaster
    let (update_tx, mut update_rx) = mpsc::channel::<Update>(100);

    // Broadcast channel for sending updates to all client handlers
    let (broadcast_tx, _) = broadcast::channel::<Update>(100);
    let broadcast_tx_clone = broadcast_tx.clone();

    // Shared state - only used to send initial state to new clients
    let state: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));
    let state_for_broadcaster = Arc::clone(&state);

    // Spawn the broadcaster task - reads from update queue and broadcasts to all clients
    tokio::spawn(async move {
        while let Some(update) = update_rx.recv().await {
            // Update the shared state
            {
                let mut state = state_for_broadcaster.write().await;
                match &update {
                    Update::Code { client_ip, code } => {
                        state.insert(client_ip.clone(), code.clone());
                    }
                    Update::Disconnected { client_ip } => {
                        state.remove(client_ip);
                    }
                }
            }
            // Broadcast to all clients
            let _ = broadcast_tx_clone.send(update);
        }
    });

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("Client connected: {}", addr);

        let state = Arc::clone(&state);
        let update_tx = update_tx.clone();
        let broadcast_rx = broadcast_tx.subscribe();

        tokio::spawn(async move {
            if let Err(e) = handle_client(socket, addr, state, update_tx, broadcast_rx).await {
                eprintln!("Error handling client {}: {}", addr, e);
            }
        });
    }
}

async fn handle_client(
    socket: TcpStream,
    addr: SocketAddr,
    state: Arc<RwLock<HashMap<String, String>>>,
    update_tx: mpsc::Sender<Update>,
    mut broadcast_rx: broadcast::Receiver<Update>,
) -> Result<()> {
    let client_addr = addr.to_string();
    let (reader, mut writer) = socket.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    // Send current state to the newly connected client
    {
        let current_state = state.read().await;
        for (ip, code) in current_state.iter() {
            let msg = ServerMessage::ClientUpdate {
                client_ip: ip.clone(),
                code: code.clone(),
            };
            let json = serde_json::to_string(&msg)? + "\n";
            writer.write_all(json.as_bytes()).await?;
        }
    }

    loop {
        tokio::select! {
            // Handle incoming messages from this client
            result = reader.read_line(&mut line) => {
                match result {
                    Ok(0) => {
                        // Client disconnected
                        println!("Client disconnected: {}", client_addr);
                        // Queue the disconnection update
                        let _ = update_tx.send(Update::Disconnected {
                            client_ip: client_addr,
                        }).await;
                        break;
                    }
                    Ok(_) => {
                        if let Ok(msg) = serde_json::from_str::<ClientMessage>(&line) {
                            match msg {
                                ClientMessage::CodeUpdate { code } => {
                                    println!("Received code update from {}", client_addr);
                                    // Queue the update for broadcasting
                                    let _ = update_tx.send(Update::Code {
                                        client_ip: client_addr.clone(),
                                        code,
                                    }).await;
                                }
                            }
                        }
                        line.clear();
                    }
                    Err(e) => {
                        eprintln!("Error reading from client {}: {}", addr, e);
                        break;
                    }
                }
            }
            // Handle broadcasts to send to this client
            result = broadcast_rx.recv() => {
                if let Ok(update) = result {
                    let msg = match update {
                        Update::Code { client_ip, code } => {
                            ServerMessage::ClientUpdate { client_ip, code }
                        }
                        Update::Disconnected { client_ip } => {
                            ServerMessage::ClientDisconnected { client_ip }
                        }
                    };
                    let json = serde_json::to_string(&msg)? + "\n";
                    writer.write_all(json.as_bytes()).await?;
                }
            }
        }
    }

    Ok(())
}

/// Client state for collaborative editing.
pub struct CollaborationClient {
    /// The TCP stream to the server.
    writer: tokio::net::tcp::OwnedWriteHalf,
    /// Current state of all clients' code.
    pub clients: Arc<RwLock<HashMap<String, String>>>,
}

impl CollaborationClient {
    /// Connects to a collaboration server.
    pub async fn connect(server_addr: &str) -> Result<(Self, tokio::task::JoinHandle<()>)> {
        let stream = TcpStream::connect(server_addr).await?;
        println!("Connected to collaboration server at {}", server_addr);

        let (reader, writer) = stream.into_split();
        let clients: Arc<RwLock<HashMap<String, String>>> = Arc::new(RwLock::new(HashMap::new()));
        let clients_clone = Arc::clone(&clients);

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
                        }
                    }
                    Err(e) => {
                        eprintln!("Error reading from server: {}", e);
                        break;
                    }
                }
            }
        });

        Ok((Self { writer, clients }, handle))
    }

    /// Sends a code update to the server.
    pub async fn send_code_update(&mut self, code: String) -> Result<()> {
        let msg = ClientMessage::CodeUpdate { code };
        let json = serde_json::to_string(&msg)? + "\n";
        self.writer.write_all(json.as_bytes()).await?;
        Ok(())
    }
}
