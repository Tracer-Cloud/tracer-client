use anyhow::Result;
use std::os::raw::{c_char, c_int, c_void};
use std::slice;
use std::sync::Arc;
use std::thread;

#[path = "types.gen.rs"]
mod types;

// Re-export types publicly
pub use types::{Event, EventHeader, EventPayload, EventType};

const TASK_COMM_LEN: usize = 16;

// FFI declarations matching bootstrap-api.h exactly - keep the C naming convention
#[repr(C)]
struct HeaderCtx {
    data: *mut EventHeaderUser,
}

#[repr(C)]
struct PayloadCtx {
    event_id: u64,
    event_type: u32, // enum event_type as u32
    data: *mut c_void,
    size: usize,
}

// Match the C structure exactly - use same layout as bootstrap.gen.h
#[repr(C, packed)]
struct EventHeaderUser {
    event_id: u64,
    event_type: u32,
    timestamp_ns: u64,
    pid: u32,
    ppid: u32,
    upid: u64,
    uppid: u64,
    comm: [c_char; TASK_COMM_LEN],
    payload: *mut c_void,
}

// FFI function - only on Linux like the working binding.rs
#[cfg(target_os = "linux")]
extern "C" {
    fn initialize(
        header_ctx: *mut HeaderCtx,
        payload_ctx: *mut PayloadCtx,
        callback: extern "C" fn(*mut HeaderCtx, *mut PayloadCtx),
    ) -> c_int;

    fn shutdown();
}

// Event listener trait
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: Event);
}

// Global listener storage
static mut GLOBAL_LISTENER: Option<Arc<dyn EventListener>> = None;
// Join-handle for the background worker
static mut SUB_THREAD: Option<thread::JoinHandle<()>> = None;

// Convert C payload to Rust types based on event type
unsafe fn convert_payload(event_type: u32, payload_ptr: *mut c_void, _size: usize) -> EventPayload {
    if payload_ptr.is_null() {
        return EventPayload::Empty;
    }

    // Convert using the generated conversion methods
    EventPayload::from_c_payload(event_type, payload_ptr)
}

// Convert C structures to Rust Event
unsafe fn convert_event(header_ctx: *mut HeaderCtx, payload_ctx: *mut PayloadCtx) -> Option<Event> {
    if header_ctx.is_null() || payload_ctx.is_null() {
        return None;
    }

    let header_ptr = (*header_ctx).data;
    if header_ptr.is_null() {
        return None;
    }

    let header_data = &*header_ptr;

    // Convert comm safely - handle potential non-null-terminated strings
    let comm_bytes = slice::from_raw_parts(header_data.comm.as_ptr() as *const u8, TASK_COMM_LEN);
    let comm_end = comm_bytes
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(TASK_COMM_LEN);
    let comm_str = String::from_utf8_lossy(&comm_bytes[..comm_end]).into_owned();

    let header = EventHeader {
        event_id: header_data.event_id,
        event_type: EventType::from(header_data.event_type),
        timestamp_ns: header_data.timestamp_ns,
        pid: header_data.pid,
        ppid: header_data.ppid,
        upid: header_data.upid,
        uppid: header_data.uppid,
        comm: comm_str,
    };

    // Convert payload
    let payload = if (*payload_ctx).data.is_null() {
        EventPayload::Empty
    } else {
        convert_payload(
            header_data.event_type,
            (*payload_ctx).data,
            (*payload_ctx).size,
        )
    };

    Some(Event { header, payload })
}

// External callback function - now properly calls the listener
extern "C" fn event_callback(header_ctx: *mut HeaderCtx, payload_ctx: *mut PayloadCtx) {
    unsafe {
        if let Some(event) = convert_event(header_ctx, payload_ctx) {
            if let Some(ref listener) = GLOBAL_LISTENER {
                listener.on_event(event);
            }
        }
    }
}

// Main subscription function
#[cfg(target_os = "linux")]
pub fn subscribe<L: EventListener + 'static>(listener: L) -> Result<()> {
    const HEADER_BUFFER_SIZE: usize = 512;
    const PAYLOAD_BUFFER_SIZE: usize = 64 * 1024;

    // Store listener globally **once**
    unsafe {
        if GLOBAL_LISTENER.is_some() {
            anyhow::bail!("subscribe() called twice");
        }
        GLOBAL_LISTENER = Some(Arc::new(listener));
    }

    // Spawn worker and return immediately
    let handle = thread::Builder::new()
        .name("ebpf-subscribe".into())
        .spawn(move || {
            // Everything lives on the worker stack
            let mut header_buf = vec![0u8; HEADER_BUFFER_SIZE];
            let mut payload_buf = vec![0u8; PAYLOAD_BUFFER_SIZE];

            let mut header_ctx = HeaderCtx {
                data: header_buf.as_mut_ptr() as *mut EventHeaderUser,
            };
            let mut payload_ctx = PayloadCtx {
                event_id: 0,
                event_type: 0,
                data: payload_buf.as_mut_ptr() as *mut c_void,
                size: PAYLOAD_BUFFER_SIZE,
            };

            // Blocks until `shutdown()` is called
            unsafe {
                let _ = initialize(&mut header_ctx, &mut payload_ctx, event_callback);
            }
        })?;

    unsafe {
        SUB_THREAD = Some(handle);
    }

    Ok(())
}

// No-op implementation for non-Linux platforms
#[cfg(not(target_os = "linux"))]
pub fn subscribe<L: EventListener + 'static>(_listener: L) -> Result<()> {
    println!("eBPF tracing is only supported on Linux");
    Ok(())
}

/// Shutdown the tracer gracefully
#[cfg(target_os = "linux")]
pub fn unsubscribe() {
    unsafe {
        shutdown();
        // Ensure the worker has finished
        if let Some(handle) = (&mut SUB_THREAD).take() {
            let _ = handle.join();
        }
        GLOBAL_LISTENER = None;
    }
}

#[cfg(not(target_os = "linux"))]
pub fn unsubscribe() {
    // No-op for non-Linux platforms
}
