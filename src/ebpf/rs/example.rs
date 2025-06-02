#[path = "binding.rs"]
mod binding;

use binding::subscribe;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let should_exit = Arc::new(AtomicBool::new(false));

    // Signal handling would be platform-specific
    let should_exit_clone = should_exit.clone();
    ctrlc::set_handler(move || {
        should_exit_clone.store(true, Ordering::Relaxed);
    })?;

    println!("Starting eBPF event logger — press Ctrl+C to stop...");

    // Start subscription in background thread
    thread::spawn(move || {
        if let Err(e) = subscribe() {
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
