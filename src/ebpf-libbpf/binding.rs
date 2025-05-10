use anyhow::Result;
use std::ffi::c_void;
use std::ptr;
use std::sync::{mpsc as std_mpsc, Arc};
use tokio::sync::mpsc::Sender;
use tokio::task;
use tracer_common::types::trigger::Trigger;

// Define the FFI interface to the C function
#[link(name = "bootstrap", kind = "static")]
extern "C" {
    // Corresponds to the initialize function in bootstrap_api.h
    fn initialize(
        buffer: *mut c_void,
        byte_count: usize,
        callback: extern "C" fn(*mut c_void, usize) -> (),
        callback_ctx: *mut c_void,
    ) -> i32;
}

// Define a struct to hold our context
struct ProcessingContext {
    events_tx: std_mpsc::Sender<Vec<Trigger>>,
    initialize_tx: std_mpsc::Sender<()>,
}

// Define a struct to hold our buffer and context
struct BufferContext {
    buffer: Vec<u8>,
    shared_context: Arc<ProcessingContext>,
}

pub fn start_processing_events(tx: Sender<Trigger>) -> Result<()> {
    // Channel for sending events from the C callback to our Rust thread
    let (events_tx, events_rx) = std_mpsc::channel::<Vec<Trigger>>();

    // Channel for signaling when to call initialize again
    let (initialize_tx, initialize_rx) = std_mpsc::channel::<()>();

    // Create our shared context
    let shared_context = Arc::new(ProcessingContext {
        events_tx,
        initialize_tx,
    });

    // Callback to be invoked by the C code, notifying Rust of writes to the shared buffer
    extern "C" fn callback_func(context_ptr: *mut c_void, filled_bytes: usize) {
        unsafe {
            // Get our context
            let context = &mut *(context_ptr as *mut BufferContext);

            // Parse events from the buffer
            let buffer_slice = &context.buffer[..filled_bytes];
            let event_size = std::mem::size_of::<Trigger>();
            let event_count = filled_bytes / event_size;
            let mut events = Vec::with_capacity(event_count);

            for i in 0..event_count {
                let event_ptr = buffer_slice.as_ptr().add(i * event_size) as *const Trigger;
                let event = ptr::read(event_ptr);
                events.push(event);
            }

            // Send the events to our channel
            let _ = context.shared_context.events_tx.send(events);

            // Signal that we should call initialize again
            let _ = context.shared_context.initialize_tx.send(());

            // Free the context
            let _ = Box::from_raw(context_ptr as *mut BufferContext);
        }
    }

    // Spawn a thread to handle calling initialize
    let shared_context_clone = shared_context.clone();
    std::thread::spawn(move || {
        loop {
            // Allocate a buffer for the C function to write to
            const BUFFER_SIZE: usize = 4096; // Adjust size as needed
            let buffer = vec![0u8; BUFFER_SIZE];

            // Create our buffer context
            let buffer_context = Box::new(BufferContext {
                buffer,
                shared_context: shared_context_clone.clone(),
            });
            let buffer_context_ptr = Box::into_raw(buffer_context);

            // Call the C function
            unsafe {
                let result = initialize(
                    (*buffer_context_ptr).buffer.as_mut_ptr() as *mut c_void,
                    (*buffer_context_ptr).buffer.len(),
                    callback_func,
                    buffer_context_ptr as *mut c_void,
                );

                if result != 0 {
                    // If initialization failed, free the context and break the loop
                    let _ = Box::from_raw(buffer_context_ptr);
                    break;
                }
            }

            // Wait for the callback to signal that we should call initialize again
            if initialize_rx.recv().is_err() {
                // If receiving fails, break the loop
                break;
            }
        }
    });

    // Task to forward events from internal std channel to external Tokio channel
    task::spawn(async move {
        while let Ok(events) = events_rx.recv() {
            for event in events {
                if tx.send(event).await.is_err() {
                    break;
                }
            }
        }
    });

    Ok(())
}
