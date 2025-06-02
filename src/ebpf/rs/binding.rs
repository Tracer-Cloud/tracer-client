use anyhow::Result;
use std::ffi::c_void;

// C structures matching bootstrap-api.h
#[repr(C)]
struct EventHeaderUser {
    event_id: u64,
    event_type: u32,
    timestamp_ns: u64,
    pid: u32,
    ppid: u32,
    upid: u64,
    uppid: u64,
    comm: [i8; 16], // TASK_COMM_LEN
    payload: *mut c_void,
}

#[repr(C)]
struct HeaderCtx {
    data: *mut EventHeaderUser,
}

#[repr(C)]
struct PayloadCtx {
    event_id: u64,
    event_type: u32,
    data: *mut c_void,
    size: usize,
}

// Define the FFI interface to the C function - only on Linux
#[cfg(target_os = "linux")]
#[link(name = "bootstrap", kind = "static")]
extern "C" {
    // Corresponds to the initialize function in bootstrap-api.h
    fn initialize(
        header_ctx: *mut HeaderCtx,
        payload_ctx: *mut PayloadCtx,
        callback: extern "C" fn(*mut HeaderCtx, *mut PayloadCtx),
    ) -> i32;
}

// Define a struct to hold our context - only needed on Linux
#[cfg(target_os = "linux")]
struct SharedContext {
    header_buf: Vec<u8>,
    payload_buf: Vec<u8>,
}

#[cfg(target_os = "linux")]
pub fn subscribe() -> Result<()> {
    const HEADER_BUFFER_SIZE: usize = 512;
    const PAYLOAD_BUFFER_SIZE: usize = 64 * 1024;

    // Create our shared context
    let shared_context = Box::new(SharedContext {
        header_buf: vec![0u8; HEADER_BUFFER_SIZE],
        payload_buf: vec![0u8; PAYLOAD_BUFFER_SIZE],
    });

    let shared_context_ptr = Box::into_raw(shared_context);

    // Callback to be invoked by the C code, notifying Rust of writes to the shared buffer
    extern "C" fn callback_func(header_ctx: *mut HeaderCtx, payload_ctx: *mut PayloadCtx) {
        unsafe {
            if header_ctx.is_null() || payload_ctx.is_null() {
                eprintln!("Error: Null context in callback");
                return;
            }

            let header_data = (*header_ctx).data;
            if header_data.is_null() {
                eprintln!("Error: Null header data");
                return;
            }

            let header = &*header_data;
            let payload = &*payload_ctx;

            println!(
                "Event: {} | Payload size: {} bytes",
                header.event_type, payload.size
            );
        }
    }

    // Call the C function - this will block until an event occurs or error
    unsafe {
        let mut header_ctx = HeaderCtx {
            data: (*shared_context_ptr).header_buf.as_mut_ptr() as *mut EventHeaderUser,
        };

        let mut payload_ctx = PayloadCtx {
            event_id: 0,
            event_type: 0,
            data: (*shared_context_ptr).payload_buf.as_mut_ptr() as *mut c_void,
            size: PAYLOAD_BUFFER_SIZE,
        };

        let result = initialize(&mut header_ctx, &mut payload_ctx, callback_func);

        // Now that initialize() has returned, we can free the context
        let _ = Box::from_raw(shared_context_ptr);

        if result != 0 {
            eprintln!("eBPF initialization failed with code: {}", result);
        }
    }

    Ok(())
}

// No-op implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub fn subscribe() -> Result<()> {
    println!("eBPF tracing is only supported on Linux");
    Ok(())
}
