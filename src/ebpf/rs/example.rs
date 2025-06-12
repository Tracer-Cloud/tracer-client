use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tracer_ebpf::{subscribe, unsubscribe, EbpfEvent, EventListener, EventPayload};

struct MyListener;

impl EventListener for MyListener {
    fn on_event(&self, event: EbpfEvent<EventPayload>) {
        println!("Received event: {:?}", event);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let should_exit = Arc::new(AtomicBool::new(false));

    // Signal handling - setup ctrl+c handler
    let should_exit_clone = should_exit.clone();
    ctrlc::set_handler(move || {
        should_exit_clone.store(true, Ordering::Relaxed);
        unsubscribe(); // Signal the C library to exit
    })?;

    let listener = MyListener;

    // Fire-and-forget - the call returns immediately
    subscribe(listener)?;

    // Main loop - wait for shutdown signal
    while !should_exit.load(Ordering::Relaxed) {
        thread::sleep(Duration::from_millis(100));
    }

    Ok(())
}
