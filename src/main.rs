mod note;
mod scale;
mod segment;
mod time;

mod lang;
mod network;
mod parser;
mod pattern;
mod superdirt;

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::RwLock;

use crate::pattern::{BoxPattern, Pattern, empty, in_parallel};
use crate::superdirt::ControlMessage;

#[derive(Parser)]
#[command(name = "leszek")]
#[command(about = "A collaborative live coding music environment")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as a collaboration server
    Server {
        /// Address to bind to
        #[arg(short, long, default_value = "0.0.0.0:9999")]
        bind: String,
    },
    /// Run as a collaboration client
    Collab {
        /// Server address to connect to
        #[arg(short, long, default_value = "127.0.0.1:9999")]
        server: String,
        /// File to track and send to the server
        file: String,
    },
    /// Run in standalone mode (default)
    Standalone {
        /// File to track and play
        file: String,
    },
}

/// Loads and parses a code string, returning the pattern to play.
fn parse_code(code: &str) -> Result<BoxPattern<ControlMessage>> {
    let parsed = parser::parse(code)?;
    lang::eval_control_pattern(parsed).ok_or_else(|| anyhow!("Evaluation error"))
}

/// Combines multiple code strings from different clients into a single pattern.
/// Uses in_parallel to play all patterns simultaneously.
fn combine_patterns(clients: &HashMap<String, String>) -> Option<Vec<BoxPattern<ControlMessage>>> {
    let patterns: Vec<BoxPattern<ControlMessage>> = clients
        .iter()
        .filter_map(|(ip, code)| {
            println!("Parsing code from client {}", ip);
            match parse_code(code) {
                Ok(pat) => Some(pat),
                Err(e) => {
                    eprintln!("Error parsing code from {}: {}", ip, e);
                    None
                }
            }
        })
        .collect();

    if patterns.is_empty() {
        Some(vec![empty().boxed()])
    } else {
        // Combine all patterns using in_parallel
        Some(vec![in_parallel(patterns.into_iter()).boxed()])
    }
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Server { bind }) => {
            run_collaboration_server(&bind);
        }
        Some(Commands::Collab { server, file }) => {
            run_collaboration_client(&server, &file);
        }
        Some(Commands::Standalone { file }) => {
            run_standalone(&file);
        }
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            std::process::exit(1);
        }
    }
}

/// Runs the collaboration server.
fn run_collaboration_server(bind: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    rt.block_on(async {
        if let Err(e) = network::run_server(bind).await {
            eprintln!("Server error: {}", e);
        }
    });
}

/// Runs the collaboration client.
fn run_collaboration_client(server_addr: &str, tracked_file: &str) {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    let tracked_file = tracked_file.to_string();

    rt.block_on(async {
        // Connect to the server
        let (mut client, mut network_update_rx, _handle) =
            match network::CollaborationClient::connect(server_addr).await {
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
                if let Ok(event) = res {
                    if event.kind.is_modify() {
                        println!("Local file changed...");
                        let _ = file_changed_tx.blocking_send(());
                    }
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
                if let Ok(content) = std::fs::read_to_string(&tracked_file) {
                    if let Err(e) = client.send_code_update(content).await {
                        eprintln!("Failed to send code update: {}", e);
                    }
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

/// Runs in standalone mode (original behavior).
fn run_standalone(tracked_file: &str) {
    let tracked_file = tracked_file.to_string();

    // Create reload flag shared between file watcher and server
    let reload_flag = Arc::new(AtomicBool::new(false));
    let reload_flag_clone = Arc::clone(&reload_flag);

    // Set up file watcher
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                // Only react to modify events
                if event.kind.is_modify() {
                    println!("File changed, signaling reload...");
                    reload_flag_clone.store(true, Ordering::SeqCst);
                }
            }
        },
        notify::Config::default().with_poll_interval(Duration::from_millis(500)),
    )
    .expect("Failed to create file watcher");

    watcher
        .watch(Path::new(&tracked_file), RecursiveMode::NonRecursive)
        .expect("Failed to watch file");

    println!("Watching {} for changes...", tracked_file);

    // Create the pattern loader for the tracked file
    let load_patterns = move || match std::fs::read_to_string(&tracked_file) {
        Ok(code) => {
            println!("Successfully loaded {}", tracked_file);
            match parse_code(&code) {
                Ok(pat) => Some(vec![pat]),
                Err(e) => {
                    eprintln!("Error: {}", e);
                    None
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to read {}: {}", tracked_file, e);
            None
        }
    };

    // Run the server (this blocks forever)
    superdirt::run_server(load_patterns, reload_flag);
}
