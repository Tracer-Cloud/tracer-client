use anyhow::Result;
use std::ffi::c_int;
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    mpsc as std_mpsc, Arc,
};
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tracer_common::types::trigger::Trigger;
use tracer_ebpf_common::process_enter::{ProcessEnterType, ProcessRawTrigger};

// Define a C-compatible event struct for validation
#[repr(C)]
struct CEvent {
    pid: c_int,
    ppid: c_int,
    event_type: c_int, // 0 for Start, 1 for Finish
    comm: [u8; 16],
    file_name: [u8; 32],
    argv: [[u8; 128]; 5],
    len: usize,
    time: u64,
}

// Validate compatibility between C event struct and Rust ProcessRawTrigger
fn validate_struct_compatibility() -> Result<(), String> {
    // Check size match
    if size_of::<CEvent>() != size_of::<ProcessRawTrigger>() {
        return Err(format!(
            "Size mismatch: CEvent is {} bytes, ProcessRawTrigger is {} bytes",
            size_of::<CEvent>(),
            size_of::<ProcessRawTrigger>()
        ));
    }

    Ok(())
}

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

// Safety limits to prevent resource exhaustion
const MAX_ITERATIONS: usize = 1000;
const BUFFER_SIZE: usize = 4096;
// const MAX_RUNTIME_SECONDS: u64 = 60; // Force exit after 1 minute if stuck

// Define a struct to hold our context
struct ProcessingContext {
    events_tx: std_mpsc::Sender<Vec<Trigger>>,
    initialize_tx: std_mpsc::Sender<()>,
    callback_count: Arc<AtomicUsize>,
}

// Define a struct to hold our buffer and context
struct BufferContext {
    buffer: Vec<u8>,
    shared_context: Arc<ProcessingContext>,
}

// Convert C event to ProcessRawTrigger safely
unsafe fn convert_to_process_raw_trigger(bytes: &[u8]) -> Result<ProcessRawTrigger, anyhow::Error> {
    // Sanity check event size
    if bytes.len() < size_of::<ProcessRawTrigger>() {
        return Err(anyhow::anyhow!(
            "Event too small: got {} bytes, expected at least {}",
            bytes.len(),
            size_of::<ProcessRawTrigger>()
        ));
    }

    // Read the C event structure
    let c_event = &*(bytes.as_ptr() as *const CEvent);

    // Create a new ProcessRawTrigger with the event data
    let trigger = ProcessRawTrigger {
        pid: c_event.pid,
        ppid: c_event.ppid,
        event_type: if c_event.event_type == 0 {
            ProcessEnterType::Start
        } else {
            ProcessEnterType::Finish
        },
        comm: c_event.comm,
        file_name: c_event.file_name,
        argv: c_event.argv,
        len: c_event.len,
        time: c_event.time,
    };

    Ok(trigger)
}

pub fn start_processing_events(tx: UnboundedSender<Trigger>) -> Result<()> {
    // Validate struct compatibility
    if let Err(e) = validate_struct_compatibility() {
        eprintln!("WARNING: Struct compatibility check failed: {}", e);
    }

    // Channel for sending events from the C callback to our Rust thread
    let (events_tx, events_rx) = std_mpsc::channel::<Vec<Trigger>>();

    // Channel for signaling when to call initialize again
    let (initialize_tx, initialize_rx) = std_mpsc::channel::<()>();

    // Counter for callback invocations
    let callback_count = Arc::new(AtomicUsize::new(0));

    // Create our shared context
    let shared_context = Arc::new(ProcessingContext {
        events_tx,
        initialize_tx,
        callback_count,
    });

    // Callback to be invoked by the C code, notifying Rust of writes to the shared buffer
    extern "C" fn callback_func(context_ptr: *mut c_void, filled_bytes: usize) {
        unsafe {
            // Get our context
            let context = &mut *(context_ptr as *mut BufferContext);

            // Track callback count for diagnostics
            let count = context
                .shared_context
                .callback_count
                .fetch_add(1, Ordering::SeqCst);

            // Emergency exit if too many callbacks occur (likely a loop)
            if count > MAX_ITERATIONS * 10 {
                eprintln!("TOO MANY CALLBACKS ({}): Emergency exit!", count);
                std::process::exit(1);
            }

            // Parse events from the buffer
            let buffer_slice = &context.buffer[..filled_bytes];

            // Sanity check on filled_bytes
            if filled_bytes == 0 {
                let _ = context.shared_context.initialize_tx.send(());
                return;
            }

            let event_size = std::mem::size_of::<ProcessRawTrigger>();

            if filled_bytes % event_size != 0 {
                eprintln!(
                    "Warning: Filled bytes {} not a multiple of event size {}",
                    filled_bytes, event_size
                );
            }

            let event_count = filled_bytes / event_size;

            let mut events = Vec::with_capacity(event_count);

            for i in 0..event_count {
                let offset = i * event_size;
                if offset + event_size > filled_bytes {
                    eprintln!("Warning: Event extends beyond buffer, skipping");
                    continue;
                }

                // Get event slice
                let event_slice = &buffer_slice[offset..offset + event_size];

                // Convert to ProcessRawTrigger safely
                match convert_to_process_raw_trigger(event_slice) {
                    Ok(raw_trigger) => {
                        // Convert from ProcessRawTrigger to Trigger
                        match (&raw_trigger).try_into() {
                            Ok(trigger) => events.push(trigger),
                            Err(e) => {
                                eprintln!("Error converting event: {:?}", e);
                                continue;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Error converting C event to ProcessRawTrigger: {:?}", e);
                        continue;
                    }
                }
            }

            // Send the events to our channel
            if !events.is_empty() {
                if let Err(e) = context.shared_context.events_tx.send(events) {
                    eprintln!("Failed to send events: {:?}", e);
                }
            }

            // Signal that we should call initialize again
            if let Err(e) = context.shared_context.initialize_tx.send(()) {
                eprintln!("Failed to send initialize signal: {:?}", e);
            }

            // Do NOT free the context here - the loop that calls initialize() will free it
            // when initialize() returns
        }
    }

    // Spawn a thread to handle calling initialize
    let shared_context_clone = shared_context.clone();
    std::thread::spawn(move || {
        // let start_time = Instant::now();
        // let mut iteration_count = 0;

        eprintln!("eBPF: Starting initialize loop");

        loop {
            // iteration_count += 1;

            // Safety check - limit iterations to prevent infinite loops
            // if iteration_count > MAX_ITERATIONS {
            //     eprintln!(
            //         "Reached maximum iterations ({}): Exiting for safety",
            //         MAX_ITERATIONS
            //     );
            //     std::process::exit(2);
            // }

            // Safety check - time limit to prevent hanging
            // if start_time.elapsed() > Duration::from_secs(MAX_RUNTIME_SECONDS) {
            //     eprintln!(
            //         "Maximum runtime ({} seconds) exceeded: Exiting for safety",
            //         MAX_RUNTIME_SECONDS
            //     );
            //     std::process::exit(3);
            // }

            // Allocate a buffer for the C function to write to
            let buffer = vec![0u8; BUFFER_SIZE];

            // Create our buffer context
            let buffer_context = Box::new(BufferContext {
                buffer,
                shared_context: shared_context_clone.clone(),
            });
            let buffer_context_ptr = Box::into_raw(buffer_context);

            eprintln!("eBPF: Calling C initialize() function...");

            // Call the C function - this will block until an event occurs or error
            unsafe {
                let result = initialize(
                    (*buffer_context_ptr).buffer.as_mut_ptr() as *mut c_void,
                    (*buffer_context_ptr).buffer.len(),
                    callback_func,
                    buffer_context_ptr as *mut c_void,
                );

                // Now that initialize() has returned, we can free the context
                // This avoids the use-after-free issue in the callback
                let _ = Box::from_raw(buffer_context_ptr);

                eprintln!("eBPF: C initialize() returned with code: {}", result);

                if result != 0 {
                    // If initialization failed, break the loop
                    eprintln!("eBPF initialization failed with code: {}", result);
                    break;
                }
            }

            eprintln!("eBPF: Waiting for signal to reinitialize...");

            // Use a timeout on receive to avoid being stuck waiting forever
            match initialize_rx.recv_timeout(Duration::from_secs(5)) {
                Ok(_) => {
                    eprintln!("eBPF: Received signal to reinitialize");
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!("eBPF: Timeout waiting for reinitialize signal, continuing anyway");
                }
                Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("Initialize channel closed, stopping eBPF processing");
                    break;
                }
            }
        }

        eprintln!("eBPF: Initialize loop exited");
    });

    // Task to forward events from internal std channel to external Tokio channel
    // Use a separate OS thread for this to ensure it works across runtimes
    std::thread::spawn(move || {
        while let Ok(events) = events_rx.recv() {
            for event in events {
                // Use unbounded_send which doesn't require async
                if let Err(e) = tx.send(event) {
                    eprintln!("Failed to send event, channel likely closed: {:?}", e);
                    return;
                }
            }
        }
    });

    Ok(())
}
