use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};
use std::slice;
use std::sync::Arc;

#[path = "types.gen.rs"]
mod types;

// Re-export types publicly
pub use types::{Event, EventHeader, EventPayload, EventType};

const TASK_COMM_LEN: usize = 16;

// FFI declarations matching bootstrap-api.h exactly
#[repr(C)]
pub struct HeaderCtx {
    pub data: *mut EventHeaderUser,
}

#[repr(C)]
pub struct PayloadCtx {
    pub event_id: u64,
    pub event_type: u32, // enum event_type as u32
    pub data: *mut c_void,
    pub size: usize,
}

#[repr(C, packed)]
pub struct EventHeaderUser {
    pub event_id: u64,
    pub event_type: u32,
    pub timestamp_ns: u64,
    pub pid: u32,
    pub ppid: u32,
    pub upid: u64,
    pub uppid: u64,
    pub comm: [c_char; TASK_COMM_LEN],
    pub payload: *mut c_void,
}

// Function pointer type matching the C callback exactly
pub type EventCallback = extern "C" fn(*mut HeaderCtx, *mut PayloadCtx);

extern "C" {
    fn initialize(
        header_ctx: *mut HeaderCtx,
        payload_ctx: *mut PayloadCtx,
        callback: EventCallback,
    ) -> c_int;
}

// High-level event listener trait
pub trait EventListener: Send + Sync {
    fn on_event(&self, event: Event);
}

// Global state for the callback
static mut GLOBAL_LISTENER: Option<Arc<dyn EventListener>> = None;

// External callback function called from C
extern "C" fn event_callback(header_ctx: *mut HeaderCtx, payload_ctx: *mut PayloadCtx) {
    unsafe {
        if let Some(listener) = &GLOBAL_LISTENER {
            if let Some(event) = convert_event(header_ctx, payload_ctx) {
                listener.on_event(event);
            }
        }
    }
}

// Convert C structures to Rust Event
unsafe fn convert_event(header_ctx: *mut HeaderCtx, payload_ctx: *mut PayloadCtx) -> Option<Event> {
    if header_ctx.is_null() || payload_ctx.is_null() {
        return None;
    }

    let header_ptr = (*header_ctx).data;
    let payload_ptr = (*payload_ctx).data;

    if header_ptr.is_null() {
        return None;
    }

    let header_data = &*header_ptr;

    // Convert header
    let comm_bytes = slice::from_raw_parts(header_data.comm.as_ptr() as *const u8, TASK_COMM_LEN);
    let comm_str = CStr::from_bytes_until_nul(comm_bytes)
        .ok()?
        .to_string_lossy()
        .into_owned();

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

    // Convert payload based on event type
    let payload = if payload_ptr.is_null() {
        convert_payload(header_data.event_type, std::ptr::null_mut(), 0)
    } else {
        convert_payload(header_data.event_type, payload_ptr, (*payload_ctx).size)
    };

    Some(Event { header, payload })
}

// Convert C payload to Rust EventPayload based on event type
unsafe fn convert_payload(event_type: u32, payload_ptr: *mut c_void, _size: usize) -> EventPayload {
    if payload_ptr.is_null() {
        return EventPayload::Empty;
    }

    // TODO: Implement payload conversion based on event type
    match event_type {
        _ => EventPayload::Empty,
    }
}

// Main subscription function
pub fn subscribe<L: EventListener + 'static>(
    listener: L,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = Arc::new(listener);

    unsafe {
        GLOBAL_LISTENER = Some(listener);
    }

    // Allocate buffers on the heap so they live beyond this function's scope
    const HEADER_BUFFER_SIZE: usize = std::mem::size_of::<EventHeaderUser>();
    const PAYLOAD_BUFFER_SIZE: usize = 256 * 1024;

    // Use Box to ensure proper lifetime management
    let header_buf = vec![0u8; HEADER_BUFFER_SIZE].into_boxed_slice();
    let payload_buf = vec![0u8; PAYLOAD_BUFFER_SIZE].into_boxed_slice();

    // Convert to raw pointers that will be valid for the C library
    let header_ptr = Box::into_raw(header_buf) as *mut EventHeaderUser;
    let payload_ptr = Box::into_raw(payload_buf) as *mut c_void;

    let mut header_ctx = HeaderCtx { data: header_ptr };

    let mut payload_ctx = PayloadCtx {
        event_id: 0,
        event_type: 0,
        data: payload_ptr,
        size: PAYLOAD_BUFFER_SIZE,
    };

    // Initialize the C library
    let result = unsafe { initialize(&mut header_ctx, &mut payload_ctx, event_callback) };

    if result != 0 {
        return Err(format!("Failed to initialize eBPF: {}", result).into());
    }

    Ok(())
}
