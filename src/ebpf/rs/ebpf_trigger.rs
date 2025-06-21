use chrono::{DateTime, Utc};
use shlex;

fn join_args(argv: &Vec<String>) -> String {
    shlex::try_join(argv.iter().map(|s| s.as_str())).unwrap_or_else(|_| argv.join(" "))
}

fn split_args(cmd: &str) -> Vec<String> {
    shlex::split(cmd).unwrap_or_else(|| cmd.split_whitespace().map(String::from).collect())
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct ProcessStartTrigger {
    /// Process ID
    pub pid: usize,
    /// Parent process ID
    pub ppid: usize,
    /// Command name (without path)
    pub comm: String,
    /// Command file name (with path)
    pub file_name: String,
    /// Command arguments (the first element is the command)
    pub argv: Vec<String>,
    /// Command string (from concatenating argv)
    pub command_string: String,
    /// Command start time
    pub started_at: DateTime<Utc>,
}

/// TODO: create a builder rather than having multiple constructors
impl ProcessStartTrigger {
    pub fn from_bpf_event(
        pid: u32,
        ppid: u32,
        comm: &str,
        argv: Vec<String>,
        timestamp_ns: u64,
    ) -> Self {
        const NS_PER_SEC: u64 = 1_000_000_000;
        Self {
            pid: pid as usize,
            ppid: ppid as usize,
            comm: comm.to_string(),
            file_name: argv.first().cloned().unwrap_or_default(),
            command_string: join_args(&argv),
            argv,
            started_at: DateTime::from_timestamp(
                (timestamp_ns / NS_PER_SEC) as i64,
                (timestamp_ns % NS_PER_SEC) as u32,
            )
            .unwrap(),
        }
    }

    pub fn from_name_and_args<A: AsRef<str>>(
        pid: usize,
        ppid: usize,
        name: &str,
        args: &[A],
    ) -> Self {
        let argv: Vec<String> = args.iter().map(|s| s.as_ref().to_string()).collect();
        Self {
            pid,
            ppid,
            comm: name.to_string(),
            command_string: join_args(&argv),
            argv,
            file_name: "".to_string(),
            started_at: Utc::now(),
        }
    }

    pub fn from_command_string(pid: usize, ppid: usize, command_string: &str) -> Self {
        let argv = split_args(command_string);
        let comm = argv.first().cloned().unwrap_or_default();
        let file_name = comm.clone();
        Self {
            pid,
            ppid,
            comm,
            argv,
            command_string: command_string.to_string(),
            file_name,
            started_at: Utc::now(),
        }
    }
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
