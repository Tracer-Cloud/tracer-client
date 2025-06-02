pub mod binding;
pub mod types;

use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessStartTrigger {
    pub pid: usize,
    pub ppid: usize,
    pub comm: String,
    pub file_name: String,
    pub argv: Vec<String>,
    pub started_at: DateTime<Utc>,
}

/// A trigger indicating a process exited. `exit_reason` is only set if known,
/// e.g., via OOM tracking or future extensions.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessEndTrigger {
    pub pid: usize,
    pub finished_at: DateTime<Utc>,
    pub exit_reason: Option<ExitReason>,
}

#[derive(Debug, Clone)]
pub struct OutOfMemoryTrigger {
    pub pid: usize,
    pub upid: u64,
    pub comm: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub enum Trigger {
    ProcessStart(ProcessStartTrigger),
    ProcessEnd(ProcessEndTrigger),
    OutOfMemory(OutOfMemoryTrigger),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum ExitReason {
    OutOfMemoryKilled,
    Signal(i32),
    Code(i32),
    Unknown,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExitReason::OutOfMemoryKilled => write!(f, "OOM Killed"),
            ExitReason::Signal(sig) => write!(f, "Signal {}", sig),
            ExitReason::Code(code) => write!(f, "Exit code {}", code),
            ExitReason::Unknown => write!(f, "Unknown"),
        }
    }
}
