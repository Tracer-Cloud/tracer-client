#[cfg(target_os = "linux")]
pub use linux::start_processing_events;
#[cfg(not(target_os = "linux"))]
pub use non_linux::start_processing_events;

#[cfg(target_os = "linux")]
mod linux {
    use crate::ebpf_trigger::Trigger;
    use anyhow::Result;
    use tokio::sync::mpsc::UnboundedSender;

    // Linux-specific imports
    use crate::types::CEvent;
    use std::ffi::c_void;
    use std::sync::{mpsc as std_mpsc, Arc};
    use std::time::Duration;

    // Define the FFI interface to the C function - only on Linux
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

    // Constants - only needed on Linux
    const BUFFER_SIZE: usize = 4096;

    // Define a struct to hold our context - only needed on Linux
    struct ProcessingContext {
        events_tx: std_mpsc::Sender<Vec<Trigger>>,
        initialize_tx: std_mpsc::Sender<()>,
    }

    // Define a struct to hold our buffer and context - only needed on Linux
    struct BufferContext {
        buffer: Vec<u8>,
        shared_context: Arc<ProcessingContext>,
    }

    pub fn start_processing_events(tx: UnboundedSender<Trigger>) -> Result<()> {
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

                let event_size = std::mem::size_of::<CEvent>();
                let event_count = filled_bytes / event_size;

                let mut events = Vec::with_capacity(event_count);

                for i in 0..event_count {
                    let offset = i * event_size;

                    // Check if we have enough bytes for a complete event
                    if offset + event_size > buffer_slice.len() {
                        eprintln!("Buffer too small for event at offset {}", offset);
                        continue;
                    }

                    // Get event slice and cast to CEvent
                    let event_slice = &buffer_slice[offset..offset + event_size];
                    let c_event = &*(event_slice.as_ptr() as *const CEvent);

                    // Convert directly from CEvent to Trigger
                    match c_event.try_into() {
                        Ok(trigger) => events.push(trigger),
                        Err(e) => {
                            eprintln!("Error converting CEvent to Trigger: {:?}", e);
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
            }
        }

        // Spawn a thread to handle calling initialize
        let shared_context_clone = shared_context.clone();
        std::thread::spawn(move || {
            loop {
                // Allocate a buffer for the C function to write to
                let buffer = vec![0u8; BUFFER_SIZE];

                // Create our buffer context
                let buffer_context = Box::new(BufferContext {
                    buffer,
                    shared_context: shared_context_clone.clone(),
                });
                let buffer_context_ptr = Box::into_raw(buffer_context);

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

                    if result != 0 {
                        // If initialization failed, break the loop
                        eprintln!("eBPF initialization failed with code: {}", result);
                        break;
                    }
                }

                // Use a timeout on receive to avoid being stuck waiting forever
                match initialize_rx.recv_timeout(Duration::from_secs(5)) {
                    Ok(_) => {}
                    Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                    Err(std_mpsc::RecvTimeoutError::Disconnected) => {
                        eprintln!("Initialize channel closed, stopping eBPF processing");
                        break;
                    }
                }
            }
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

    #[cfg(test)]
    mod tests {
        use crate::ebpf_trigger::{ProcessEndTrigger, ProcessStartTrigger, Trigger};
        use std::process::Command;
        use tokio::sync::mpsc;

        #[tokio::test]
        async fn test_exit_code() {
            let (tx, mut rx) = mpsc::unbounded_channel::<Trigger>();
            super::start_processing_events(tx).unwrap();

            // run a process that exits with an error
            let mut handle = Command::new("bash")
                .arg("-c")
                .arg("\"sleep 10; exit 1\"")
                .spawn()
                .unwrap();
            let status = handle.wait().unwrap();
            assert!(!status.success());

            // check that we got exec and exit events
            const MAX_TRIES: usize = 10;
            let mut tries: usize = 0;
            let mut exec_trigger: Option<ProcessStartTrigger> = None;
            let mut exit_trigger: Option<ProcessEndTrigger> = None;
            loop {
                match tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await {
                    Ok(Some(event)) => match event {
                        Trigger::ProcessStart(trigger)
                            if trigger.command_string == "bash -c \"sleep 10; exit 1\"" =>
                        {
                            exec_trigger = Some(trigger)
                        }
                        Trigger::ProcessEnd(trigger)
                            if exec_trigger
                                .as_ref()
                                .map(|t| t.pid == trigger.pid)
                                .unwrap_or(false) =>
                        {
                            exit_trigger = Some(trigger);
                            break;
                        }
                        _ => {}
                    },
                    Ok(None) => break,
                    _ => (),
                }
                tries += 1;
                if tries > MAX_TRIES {
                    break;
                }
            }
            assert!(exec_trigger.is_some());
            assert!(exit_trigger.is_some());
            assert_eq!(exit_trigger.unwrap().exit_reason.unwrap().code, 1);
        }
    }
}

// No-op implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
mod non_linux {
    use crate::ebpf_trigger::Trigger;
    use anyhow::Result;
    use tokio::sync::mpsc::UnboundedSender;

    pub fn start_processing_events(_tx: UnboundedSender<Trigger>) -> Result<()> {
        eprintln!("eBPF functionality is only supported on Linux");
        Ok(())
    }
}
