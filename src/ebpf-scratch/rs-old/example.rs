use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[path = "binding.rs"]
mod binding;

use binding::{subscribe, Event, EventListener};

struct EventPrinter;

impl EventListener for EventPrinter {
    fn on_event(&self, event: Event) {
        let json = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
        println!("{}", json);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let should_exit = Arc::new(AtomicBool::new(false));

    // Signal handling would be platform-specific, simplified for demo
    let should_exit_clone = should_exit.clone();
    ctrlc::set_handler(move || {
        should_exit_clone.store(true, Ordering::Relaxed);
    })?;

    println!("Starting eBPF event logger – press Ctrl+C to stop...");

    let listener = EventPrinter;

    // Start subscription in background thread
    thread::spawn(move || {
        if let Err(e) = subscribe(listener) {
            eprintln!("Failed to start subscription: {}", e);
        }
    });

    // Main loop
    while !should_exit.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    println!("\nShutting down...");
    Ok(())
}
