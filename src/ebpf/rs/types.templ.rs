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
use std::os::raw::c_void;
use std::slice;

// templ_start:event_type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[repr(u32)]
pub enum EventType {
    // Generated variants will be inserted here
    // Add unknown variant for robustness
    Unknown(u32),
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
#[derive(Debug, Clone, Deserialize)]
pub struct Event {
    pub header: EventHeader,
    pub payload: EventPayload,
}

// Custom serialization for Event to match example.cpp JSON format
impl Serialize for Event {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;

        let mut map = serializer.serialize_map(None)?;

        // Serialize header fields directly
        map.serialize_entry("event_id", &self.header.event_id)?;
        map.serialize_entry("event_type", &self.header.event_type.as_str())?;
        map.serialize_entry("timestamp_ns", &self.header.timestamp_ns)?;
        map.serialize_entry("pid", &self.header.pid)?;
        map.serialize_entry("ppid", &self.header.ppid)?;
        map.serialize_entry("upid", &self.header.upid)?;
        map.serialize_entry("uppid", &self.header.uppid)?;
        map.serialize_entry("comm", &self.header.comm)?;

        // Only add payload if it's not empty
        if !matches!(self.payload, EventPayload::Empty) {
            map.serialize_entry("payload", &self.payload)?;
        }

        map.end()
    }
}

// templ_start:event_payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    Empty,
}
// templ_end:event_payload

// templ_start:payload_structs
// templ_end:payload_structs

// C structures for payload conversion
#[repr(C, packed)]
struct FlexBuf {
    byte_length: u32,
    data: *mut i8,
}

// templ_start:payload_conversion
impl EventPayload {
    pub unsafe fn from_c_payload(event_type: u32, payload_ptr: *mut c_void) -> Self {
        match event_type {
            _ => EventPayload::Empty,
        }
    }
}
// templ_end:payload_conversion

// Helper function to convert C string from FlexBuf
unsafe fn flex_buf_to_string(flex_buf: &FlexBuf) -> String {
    if flex_buf.data.is_null() || flex_buf.byte_length == 0 {
        return String::new();
    }

    let slice = slice::from_raw_parts(flex_buf.data as *const u8, flex_buf.byte_length as usize);
    // Find null terminator if present
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    String::from_utf8_lossy(&slice[..len]).into_owned()
}

// Helper function to convert C string array from FlexBuf (null-separated strings)
unsafe fn flex_buf_to_string_array(flex_buf: &FlexBuf) -> Vec<String> {
    if flex_buf.data.is_null() || flex_buf.byte_length == 0 {
        return Vec::new();
    }

    let slice = slice::from_raw_parts(flex_buf.data as *const u8, flex_buf.byte_length as usize);
    let mut result = Vec::new();
    let mut start = 0;

    for i in 0..=slice.len() {
        if i == slice.len() || slice[i] == 0 {
            if i > start {
                let string = String::from_utf8_lossy(&slice[start..i]).into_owned();
                result.push(string);
            }
            start = i + 1;
        }
    }

    result
}

// templ_start:event_type_from_u32
impl From<u32> for EventType {
    fn from(value: u32) -> Self {
        match value {
            unknown => EventType::Unknown(unknown),
        }
    }
}
// templ_end:event_type_from_u32

// templ_start:event_type_to_string
impl EventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::Unknown(_) => "unknown",
        }
    }
}
// templ_end:event_type_to_string

// Custom Serialize implementation to handle Unknown variant properly
impl Serialize for EventType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            EventType::Unknown(val) => serializer.serialize_str(&format!("unknown({})", val)),
            _ => serializer.serialize_str(self.as_str()),
        }
    }
}
