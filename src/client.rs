//! Collaboration client module.
//!
//! This module provides client-side functionality for collaborative editing,
//! including connecting to servers, sending updates, and receiving updates
//! from other clients.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::sync::{RwLock, mpsc};

use crate::eval;
use crate::network::{ClientMessage, ServerMessage};
use crate::pattern::{BoxPattern, Pattern, in_parallel};
use crate::superdirt::{self, ControlMessage};

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

/// Combines multiple code strings from different clients into a single pattern.
/// Uses in_parallel to play all patterns simultaneously.
fn combine_patterns(clients: &HashMap<String, String>) -> BoxPattern<ControlMessage> {
    let patterns: Vec<BoxPattern<ControlMessage>> = clients
        .iter()
        .filter_map(|(ip, code)| {
            println!("Parsing code from client {}", ip);
            match eval::parse_and_eval_code(code) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    eprintln!("Error parsing code from {}: {}", ip, e);
                    None
                }
            }
        })
        .collect();
    in_parallel(patterns.into_iter()).boxed()
}

/// Runs the collaboration client.
pub fn run_collaboration_client(server_addr: &str, tracked_file: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let tracked_file = tracked_file.to_string();

    rt.block_on(async {
        // Connect to the server
        let (mut client, mut network_update_rx, _handle) =
            match CollaborationClient::connect(server_addr).await {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("Failed to connect to server: {}", e);
                    return;
                }
            };

        // Shared state for the combined patterns from all clients
        let combined_clients: Arc<RwLock<HashMap<String, String>>> = Arc::clone(&client.clients);

        // Create reload flag shared between file watcher, network updates, and SuperDirt server
        let reload_flag = Arc::new(AtomicBool::new(false));
        let reload_flag_network = Arc::clone(&reload_flag);

        // Channel for file watcher to notify about changes
        let (file_changed_tx, mut file_changed_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Set up file watcher to watch local file
        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res
                    && event.kind.is_modify()
                {
                    println!("Local file changed...");
                    let _ = file_changed_tx.blocking_send(());
                }
            },
            notify::Config::default().with_poll_interval(Duration::from_millis(500)),
        )
        .expect("Failed to create file watcher");

        watcher
            .watch(Path::new(&tracked_file), RecursiveMode::NonRecursive)
            .expect("Failed to watch file");

        println!("Watching {} for changes...", tracked_file);

        // Spawn a task to send local file updates to the server
        // Sends initial content immediately, then waits for watcher notifications
        tokio::spawn(async move {
            loop {
                if let Ok(content) = std::fs::read_to_string(&tracked_file)
                    && let Err(e) = client.send_code_update(content).await
                {
                    eprintln!("Failed to send code update: {}", e);
                }

                // Wait for next file change (first iteration runs immediately)
                if file_changed_rx.recv().await.is_none() {
                    break;
                }
            }
        });

        // Spawn a task to watch for network updates (notified via channel)
        tokio::spawn(async move {
            while network_update_rx.recv().await.is_some() {
                println!("Network state updated, signaling reload...");
                reload_flag_network.store(true, Ordering::SeqCst);
            }
        });

        // Create the pattern loader that combines all clients' patterns
        let load_combined = {
            let combined_clients = Arc::clone(&combined_clients);
            move || {
                // Use tokio's Handle to run async code from sync context
                let handle = tokio::runtime::Handle::current();
                let clients = handle.block_on(async { combined_clients.read().await.clone() });
                println!("Loading patterns from {} clients", clients.len());
                combine_patterns(&clients)
            }
        };

        // Run the SuperDirt server in a blocking task
        let reload_flag_superdirt = Arc::clone(&reload_flag);
        tokio::task::spawn_blocking(move || {
            superdirt::run_server(load_combined, reload_flag_superdirt);
        })
        .await
        .expect("SuperDirt server panicked");
    });
}
