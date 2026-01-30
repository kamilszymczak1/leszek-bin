mod client;
mod note;
mod scale;
mod segment;
mod server;
mod time;

mod lang;
mod network;
mod parser;
mod pattern;
mod superdirt;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::pattern::BoxPattern;
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
    lang::eval_control_pattern(parsed)
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Server { bind }) => {
            run_collaboration_server(&bind);
        }
        Some(Commands::Collab { server, file }) => {
            client::run_collaboration_client(&server, &file);
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
        if let Err(e) = server::run_server(bind).await {
            eprintln!("Server error: {}", e);
        }
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
