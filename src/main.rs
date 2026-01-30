mod note;
mod scale;
mod segment;
mod time;

mod lang;
mod parser;
mod pattern;
mod superdirt;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::pattern::{BoxPattern, Pattern, cycled, fastcat, slowcat};
use crate::superdirt::{ControlMessage, run_server};

const DEMO_FILE: &str = "demo.code";

/// Loads and parses the demo file, returning the patterns to play.
fn load_patterns() -> Option<Vec<BoxPattern<ControlMessage>>> {
    match std::fs::read_to_string(DEMO_FILE) {
        Ok(code) => match parser::parse(&code) {
            Ok(parsed) => match lang::eval_control_pattern(parsed) {
                Some(pat) => {
                    println!("Successfully loaded {}", DEMO_FILE);
                    Some(vec![pat])
                }
                None => {
                    eprintln!("Evaluation error");
                    None
                }
            },
            Err(e) => {
                eprintln!("Parse error: {}", e);
                None
            }
        },
        Err(e) => {
            eprintln!("Failed to read {}: {}", DEMO_FILE, e);
            None
        }
    }
}

fn main() {
    fn s(name: &'static str) -> pattern::BoxPattern<ControlMessage> {
        pattern::cycled(ControlMessage::sound(name)).boxed()
    }

    println!(
        "{}",
        pattern::display_pattern(
            &fastcat(
                [
                    cycled(0).boxed(),
                    slowcat([cycled(1), cycled(2)].into_iter()).boxed()
                ]
                .into_iter()
            )
            .boxed()
        )
    );
    println!(
        "{}",
        pattern::display_pattern(
            &lang::eval_pattern(parser::parse("cat([1, 2])").unwrap()).unwrap()
        )
    );
    let _pat1 =
        fastcat([s("bd"), slowcat([s("sn"), s("hh")].into_iter()).boxed()].into_iter()).boxed();
    let _pat2 = pattern::cycled(ControlMessage::sound("arpy")).boxed();

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
        .watch(Path::new(DEMO_FILE), RecursiveMode::NonRecursive)
        .expect("Failed to watch file");

    println!("Watching {} for changes...", DEMO_FILE);

    // Run the server (this blocks forever)
    run_server(load_patterns, reload_flag);
}
