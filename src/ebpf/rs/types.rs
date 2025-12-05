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
    // We'll access specific payloads by casting
    pub payload: [u8; 2048], // Size should be sufficient for largest payload
}

// Keep the helper function from the original code
pub fn from_bpf_str(s: &[u8]) -> anyhow::Result<&str> {
    let zero_pos = s.iter().position(|&x| x == 0);
    let s = match zero_pos {
        Some(pos) => &s[..pos],
        None => s,
    };
    Ok(std::str::from_utf8(s)?)
}

pub fn env_val(s: &[u8]) -> Option<String> {
    from_bpf_str(s)
        .ok()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

// Implement TryInto for CEvent to convert directly to Trigger
impl TryInto<ebpf_trigger::Trigger> for &CEvent {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<ebpf_trigger::Trigger, Self::Error> {
        match self.event_type {
            EVENT__SCHED__SCHED_PROCESS_EXEC => {
                // Access the exec payload by casting
                let payload_ptr = self.payload.as_ptr() as *const SchedProcessExecPayload;
                let payload = unsafe { &*payload_ptr };

                // Get command name
                let comm = from_bpf_str(&payload.comm)?;

                // Collect arguments
                let mut args = Vec::new();
                for i in 0..payload.argc as usize {
                    if i >= MAX_ARR_LEN {
                        break;
                    }
                    args.push(from_bpf_str(&payload.argv[i])?.to_string());
                }

                Ok(ebpf_trigger::Trigger::ProcessStart(
                    ebpf_trigger::ProcessStartTrigger::from_bpf_event(
                        self.pid,
                        self.ppid,
                        comm,
                        args,
                        self.timestamp_ns,
                    ),
                ))
            }
            EVENT__SCHED__SCHED_PROCESS_EXIT => {
                // Access the exec payload by casting
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
                // Access the common process name (`comm`) by casting payload
                let comm = from_bpf_str(&self.payload[..TASK_COMM_LEN])?;

                Ok(ebpf_trigger::Trigger::OutOfMemory(
                    ebpf_trigger::OutOfMemoryTrigger {
                        pid: self.pid as usize,
                        upid: self.upid,
                        comm: comm.to_string(),
                        timestamp: chrono::DateTime::from_timestamp(
                            (self.timestamp_ns / 1_000_000_000) as i64,
                            (self.timestamp_ns % 1_000_000_000) as u32,
                        )
                        .unwrap(),
                    },
                ))
            }
            EVENT__SYSCALL__SYS_ENTER_OPENAT => {
                // Casting the payload
                let payload_ptr = self.payload.as_ptr() as *const SysEnterOpenAtPayload;
                let payload = unsafe { &*payload_ptr };
                let pid = self.pid;

                // Extracting the filename
                let filename = from_bpf_str(&payload.filename)?.to_string();

                // Getting the size of the file in bytes
                let size_bytes = get_file_size(pid, &filename).unwrap_or(-1);
                let file_full_path = get_file_full_path(pid, &filename);

                Ok(ebpf_trigger::Trigger::FileOpen(
                    ebpf_trigger::FileOpenTrigger {
                        pid: pid as usize,
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
            _ => Err(anyhow::anyhow!("Unsupported event type")),
        }
    }
}
