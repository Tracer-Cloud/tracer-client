// templ_start:file_description
/* ========================================================================== */
/*                            TEMPLATE FILE                                   */
/* ========================================================================== */
/*                                                                            */
/*  This template is used to generate types.gen.rs                           */
/*  REGENERATE AFTER EDITING - changes will have no effect                    */
/*                                                                            */
/*  Generator: ebpf/typegen/typegen.rs                                        */
/*  Template:  ebpf/rs/types.templ.rs                                         */
/*  Config:    ebpf/typegen/events.toml                                       */
/*                                                                            */
/*  To regenerate: `cd tracer-client/src/ebpf/c && make` (fast)               */
/*  Alternative:   `cd tracer-client && cargo build` (slower)                 */
/*                                                                            */
/* ========================================================================== */
// templ_end:file_description

use serde::{Deserialize, Serialize};

// templ_start:event_type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u32)]
pub enum EventType {
}
// templ_end:event_type

// High-level event header (converted from C)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventHeader {
    pub event_id: u64,
    pub event_type: EventType,
    pub timestamp_ns: u64,
    pub pid: u32,
    pub ppid: u32,
    pub upid: u64,
    pub uppid: u64,
    pub comm: String,
}

// Combined event structure with header and typed payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    pub header: EventHeader,
    pub payload: EventPayload,
}

// templ_start:event_payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    Empty,
}
// templ_end:event_payload

// templ_start:payload_structs
// templ_end:payload_structs

// Helper trait for converting from C representations
pub trait FromC<T> {
    fn from_c(value: T) -> Self;
}

// templ_start:event_type_from_u32
impl From<u32> for EventType {
    fn from(value: u32) -> Self {
        match value {
            _ => panic!("Unknown event type: {}", value),
        }
    }
}
// templ_end:event_type_from_u32

// templ_start:event_type_to_string

// templ_end:event_type_to_string
