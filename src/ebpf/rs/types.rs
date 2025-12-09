use crate::ebpf_trigger;
use crate::utils::{get_file_full_path, get_file_size};

// CEvent must be kept in-sync with bootstrap.h types
pub const TASK_COMM_LEN: usize = 16;
pub const MAX_ARR_LEN: usize = 16;
pub const MAX_STR_LEN: usize = 128;
pub const MAX_ENV_LEN: usize = 1;
pub const ENV_KEYS: [&str; MAX_ENV_LEN] = ["TRACER_TRACE_ID"];

// Event type constants matching enum event_type
pub const EVENT__SCHED__SCHED_PROCESS_EXEC: u32 = 0;
pub const EVENT__SCHED__SCHED_PROCESS_EXIT: u32 = 1;
pub const EVENT__SCHED__PSI_MEMSTALL_ENTER: u32 = 16;
pub const EVENT__SYSCALL__SYS_ENTER_OPENAT: u32 = 1024;
pub const EVENT__SYSCALL__SYS_EXIT_OPENAT: u32 = 1025;
pub const EVENT__SYSCALL__SYS_ENTER_READ: u32 = 1026;
pub const EVENT__SYSCALL__SYS_EXIT_READ: u32 = 1027;
pub const EVENT__SYSCALL__SYS_ENTER_WRITE: u32 = 1028;
pub const EVENT__SYSCALL__SYS_EXIT_WRITE: u32 = 1029;
pub const EVENT__VMSCAN__MM_VMSCAN_DIRECT_RECLAIM_BEGIN: u32 = 2048;
pub const EVENT__OOM__MARK_VICTIM: u32 = 3072;
pub const EVENT__PYTHON__FUNCTION_ENTRY: u32 = 4096;
pub const EVENT__PYTHON__FUNCTION_EXIT: u32 = 4097;

// Define payload structs for the events we care about
#[repr(C, packed)]
pub struct SchedProcessExecPayload {
    pub comm: [u8; TASK_COMM_LEN],
    pub argc: u32,
    pub argv: [[u8; MAX_STR_LEN]; MAX_ARR_LEN],
    pub env_found_mask: u32,
    pub env_values: [[u8; MAX_STR_LEN]; MAX_ENV_LEN],
}

pub struct SchedProcessExitPayload {
    pub status: u16,
}

// struct syscall__sys_enter_openat__payload in bootstrap.h
#[repr(C, packed)]
pub struct SysEnterOpenAtPayload {
    pub dfd: i32,
    pub filename: [u8; MAX_STR_LEN],
    pub flags: i32,
    pub mode: i32,
}

// Must match struct python__function_entry__payload in bootstrap.h
#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct PythonFunctionEntryPayload {
    pub filename: [u8; MAX_STR_LEN],
    pub function_name: [u8; MAX_STR_LEN],
    pub line_number: i32,
    pub entry_time_ns: u64,
}

// Must match struct python__function_exit__payload in bootstrap.h
#[repr(C, packed)]
#[derive(Debug, Clone)]
pub struct PythonFunctionExitPayload {
    pub filename: [u8; MAX_STR_LEN],
    pub function_name: [u8; MAX_STR_LEN],
    pub line_number: i32,
    pub entry_time_ns: u64,
    pub duration_ns: u64,
}

// Define the CEvent struct to match the memory layout of the C struct
#[repr(C, packed)]
pub struct CEvent {
    // Common fields
    pub event_type: u32,
    pub timestamp_ns: u64,
    pub pid: u32,
    pub ppid: u32,
    pub upid: u64,
    pub uppid: u64,

    // Payload - using a byte array large enough to hold any payload
    pub payload: [u8; 2048],
}

// --------------------------------------------------------------------------
// Robust String Parser
// --------------------------------------------------------------------------
// Uses `from_utf8_lossy` to prevent crashes on garbage BPF memory.
// Filters out non-printable characters for cleaner output.
pub fn from_bpf_str(s: &[u8]) -> anyhow::Result<String> {
    // Find the first null byte (0). If not found, use the full length.
    let len = s.iter().position(|&x| x == 0).unwrap_or(s.len());

    // Slice only the valid data
    let valid_slice = &s[..len];

    // Convert to String, replacing bad bytes with replacement character
    let raw_string = String::from_utf8_lossy(valid_slice);

    // Filter to only printable ASCII and common Unicode
    // This removes control characters and other garbage
    let cleaned: String = raw_string
        .chars()
        .filter(|c| {
            // Keep printable ASCII (including space)
            (*c >= ' ' && *c <= '~') ||
                // Keep common path/filename characters
                *c == '/' || *c == '.' || *c == '_' || *c == '-' ||
                // Keep angle brackets for <frozen ...> modules
                *c == '<' || *c == '>'
        })
        .collect();

    Ok(cleaned)
}

pub fn env_val(s: &[u8]) -> Option<String> {
    from_bpf_str(s)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Format duration in human-readable form
pub fn format_duration_ns(duration_ns: u64) -> String {
    if duration_ns == 0 {
        "N/A".to_string()
    } else if duration_ns < 1_000 {
        format!("{}ns", duration_ns)
    } else if duration_ns < 1_000_000 {
        format!("{:.2}µs", duration_ns as f64 / 1_000.0)
    } else if duration_ns < 1_000_000_000 {
        format!("{:.2}ms", duration_ns as f64 / 1_000_000.0)
    } else {
        format!("{:.3}s", duration_ns as f64 / 1_000_000_000.0)
    }
}

// Implement TryInto for CEvent to convert directly to Trigger
impl TryInto<ebpf_trigger::Trigger> for &CEvent {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ebpf_trigger::Trigger, Self::Error> {
        match self.event_type {
            EVENT__SCHED__SCHED_PROCESS_EXEC => {
                let payload_ptr = self.payload.as_ptr() as *const SchedProcessExecPayload;
                let payload = unsafe { &*payload_ptr };

                let comm = from_bpf_str(&payload.comm)?;

                let mut args = Vec::new();
                for i in 0..payload.argc as usize {
                    if i >= MAX_ARR_LEN {
                        break;
                    }
                    args.push(from_bpf_str(&payload.argv[i])?);
                }

                Ok(ebpf_trigger::Trigger::ProcessStart(
                    ebpf_trigger::ProcessStartTrigger::from_bpf_event(
                        self.pid,
                        self.ppid,
                        comm.as_str(),
                        args,
                        self.timestamp_ns,
                    ),
                ))
            }
            EVENT__SCHED__SCHED_PROCESS_EXIT => {
                let payload_ptr = self.payload.as_ptr() as *const SchedProcessExitPayload;
                let payload = unsafe { &*payload_ptr };

                Ok(ebpf_trigger::Trigger::ProcessEnd(
                    ebpf_trigger::ProcessEndTrigger {
                        pid: self.pid as usize,
                        finished_at: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                            .unwrap(),
                        exit_reason: Some((payload.status as i64).into()),
                    },
                ))
            }
            EVENT__OOM__MARK_VICTIM => {
                let comm = from_bpf_str(&self.payload[..TASK_COMM_LEN])?;

                Ok(ebpf_trigger::Trigger::OutOfMemory(
                    ebpf_trigger::OutOfMemoryTrigger {
                        pid: self.pid as usize,
                        upid: self.upid,
                        comm,
                        timestamp: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                            .unwrap(),
                    },
                ))
            }
            EVENT__SYSCALL__SYS_ENTER_OPENAT => {
                let payload_ptr = self.payload.as_ptr() as *const SysEnterOpenAtPayload;
                let payload = unsafe { &*payload_ptr };
                let pid = self.pid;

                let filename = from_bpf_str(&payload.filename)?;

                let size_bytes = get_file_size(pid, &filename).unwrap_or(-1);
                let file_full_path = get_file_full_path(pid, &filename);

                Ok(ebpf_trigger::Trigger::FileOpen(
                    ebpf_trigger::FileOpenTrigger {
                        pid,
                        filename,
                        size_bytes,
                        timestamp: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                            .unwrap(),
                        file_full_path,
                    },
                ))
            }
            EVENT__PYTHON__FUNCTION_ENTRY => {
                let payload_ptr = self.payload.as_ptr() as *const PythonFunctionEntryPayload;
                let payload = unsafe { &*payload_ptr };

                let pid = self.pid;
                let filename = from_bpf_str(&payload.filename)?;
                let function_name = from_bpf_str(&payload.function_name)?;
                let line_number = payload.line_number;
                let entry_time_ns = payload.entry_time_ns;

                println!("PythonFunctionEntry: {:?}:{:?}:{:?}, entry_time_ns: {:?}", filename, function_name, line_number, entry_time_ns);

                Ok(ebpf_trigger::Trigger::PythonFunctionEntry(
                    ebpf_trigger::PythonFunctionEntryTrigger {
                        pid,
                        filename,
                        function_name,
                        line_number,
                        entry_time_ns,
                        timestamp: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                            .unwrap(),
                    },
                ))
            }
            EVENT__PYTHON__FUNCTION_EXIT => {
                let payload_ptr = self.payload.as_ptr() as *const PythonFunctionExitPayload;
                let payload = unsafe { &*payload_ptr };

                let pid = self.pid;
                let filename = from_bpf_str(&payload.filename)?;
                let function_name = from_bpf_str(&payload.function_name)?;
                let line_number = payload.line_number;
                let entry_time_ns = payload.entry_time_ns;
                let duration_ns = payload.duration_ns;

                println!("PythonFunctionExit: {:?}:{:?}:{:?}, duration_ns: {:?}", filename, function_name, line_number, duration_ns);

                Ok(ebpf_trigger::Trigger::PythonFunctionExit(
                    ebpf_trigger::PythonFunctionExitTrigger {
                        pid,
                        filename,
                        function_name,
                        line_number,
                        entry_time_ns,
                        duration_ns,
                        timestamp: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                            .unwrap(),
                    },
                ))
            }
            _ => Err(anyhow::anyhow!("Unsupported event type")),
        }
    }
}